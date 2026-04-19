use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use ical::IcalParser;
use rusqlite::params;
use std::collections::{HashMap, HashSet};
use std::io::BufReader;
use tauri::State;

use crate::commands::settings::now;
use crate::commands::settings_ical::load_settings_ical;
use crate::state::AppState;
use crate::timeline::{TimelineEvent, TimelineEventSource};

const CACHE_TTL_MS: i64 = 10 * 60 * 1000; // 10 minutes

pub(crate) fn fetch_ical_cached(state: &State<'_, AppState>, url: &str) -> Result<String, String> {
    let cutoff = now() - CACHE_TTL_MS;

    {
        let conn = state.db.lock().map_err(|_| "DB lock failed".to_string())?;
        let cached: Option<String> = conn
            .query_row(
                "SELECT content FROM calendar_ical_cache WHERE url = ?1 AND fetched_at > ?2",
                params![url, cutoff],
                |row| row.get(0),
            )
            .ok();
        if let Some(content) = cached {
            return Ok(content);
        }
    }

    let content = ureq::get(url)
        .set("Accept", "text/calendar")
        .call()
        .map_err(|e| format!("iCal fetch: {e}"))?
        .into_string()
        .map_err(|e| format!("iCal read: {e}"))?;

    {
        let conn = state.db.lock().map_err(|_| "DB lock failed".to_string())?;
        let _ = conn.execute(
            "INSERT OR REPLACE INTO calendar_ical_cache (url, content, fetched_at) VALUES (?1, ?2, ?3)",
            params![url, &content, now()],
        );
    }

    Ok(content)
}

pub(crate) fn parse_ics_datetime_pub(value: &str) -> Option<DateTime<Local>> {
    parse_ics_datetime(value)
}

struct ParsedEvent {
    uid: String,
    summary: String,
    url: Option<String>,
    dtstart_val: String,
    dtstart_tzid: Option<String>,
    is_all_day: bool,
    rrule: Option<String>,
    exdates: Vec<String>,
    recurrence_id_ts: Option<i64>,
}

fn parse_events_from_ics(body: &str) -> Vec<ParsedEvent> {
    let reader = BufReader::new(body.as_bytes());
    let parser = IcalParser::new(reader);
    let mut events = Vec::new();

    for calendar in parser.flatten() {
        for event in &calendar.events {
            let get = |name: &str| -> Option<String> {
                event
                    .properties
                    .iter()
                    .find(|p| p.name == name)
                    .and_then(|p| p.value.clone())
            };

            let dtstart_prop = event.properties.iter().find(|p| p.name == "DTSTART");
            let dtstart_val = match dtstart_prop.and_then(|p| p.value.as_deref()) {
                Some(v) => v.to_string(),
                None => continue,
            };

            let dtstart_tzid = dtstart_prop
                .and_then(|p| p.params.as_ref())
                .and_then(|ps| ps.iter().find(|(k, _)| k == "TZID"))
                .and_then(|(_, v)| v.first().cloned());

            let is_all_day = dtstart_prop
                .and_then(|p| p.params.as_ref())
                .and_then(|ps| ps.iter().find(|(k, _)| k == "VALUE"))
                .map(|(_, v)| v.iter().any(|s| s == "DATE"))
                .unwrap_or(false)
                || !dtstart_val.contains('T');

            let rrule = get("RRULE");

            let exdates: Vec<String> = event
                .properties
                .iter()
                .filter(|p| p.name == "EXDATE")
                .filter_map(|p| p.value.clone())
                .collect();

            let recurrence_id_ts = get("RECURRENCE-ID")
                .as_deref()
                .and_then(parse_ics_datetime)
                .map(|dt| dt.timestamp());

            let uid = get("UID").unwrap_or_else(|| format!("no-uid:{dtstart_val}"));

            events.push(ParsedEvent {
                uid,
                summary: get("SUMMARY").unwrap_or_else(|| "(No title)".into()),
                url: get("URL"),
                dtstart_val,
                dtstart_tzid,
                is_all_day,
                rrule,
                exdates,
                recurrence_id_ts,
            });
        }
    }

    events
}

fn expand_rrule_for_day(
    dtstart_val: &str,
    dtstart_tzid: Option<&str>,
    rrule_val: &str,
    exdates: &[String],
    day_naive: NaiveDate,
) -> Vec<DateTime<Local>> {
    use rrule::RRuleSet;

    let tz_prefix = dtstart_tzid
        .map(|tz| format!(";TZID={tz}"))
        .unwrap_or_default();

    let mut ical_str = format!("DTSTART{tz_prefix}:{dtstart_val}\nRRULE:{rrule_val}\n");
    for exdate in exdates {
        ical_str.push_str(&format!("EXDATE:{exdate}\n"));
    }

    let rrule_set: RRuleSet = match ical_str.parse() {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    let day_start_local = match Local
        .from_local_datetime(&day_naive.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
    {
        Some(dt) => dt,
        None => return vec![],
    };
    let day_end_local = match Local
        .from_local_datetime(&day_naive.and_hms_opt(23, 59, 59).unwrap())
        .earliest()
    {
        Some(dt) => dt,
        None => return vec![],
    };

    let after_tz: DateTime<rrule::Tz> = day_start_local.with_timezone(&rrule::Tz::UTC);
    let before_tz: DateTime<rrule::Tz> = day_end_local.with_timezone(&rrule::Tz::UTC);

    rrule_set
        .after(after_tz)
        .before(before_tz)
        .all(50)
        .dates
        .into_iter()
        .map(|dt| {
            let utc = Utc.from_utc_datetime(&dt.naive_utc());
            utc.with_timezone(&Local)
        })
        .filter(|dt| dt.date_naive() == day_naive)
        .collect()
}

pub(super) fn events_for_day(
    state: &State<'_, AppState>,
    day: &str,
) -> Result<Vec<(i64, TimelineEvent)>, String> {
    let settings = load_settings_ical(state);

    if !settings.enabled {
        return Ok(Vec::new());
    }

    let urls: Vec<String> = settings
        .urls
        .into_iter()
        .filter(|u| !u.trim().is_empty())
        .collect();

    if urls.is_empty() {
        return Ok(Vec::new());
    }

    let day_naive =
        NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|_| format!("Invalid date: {day}"))?;

    let mut rows: Vec<(i64, TimelineEvent)> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    for url in &urls {
        let body = fetch_ical_cached(state, url.trim())?;
        let parsed = parse_events_from_ics(&body);

        // Collect RECURRENCE-ID overrides: uid -> set of timestamps with an explicit instance.
        let mut overridden_ts: HashMap<String, HashSet<i64>> = HashMap::new();
        for ev in &parsed {
            if let Some(ts) = ev.recurrence_id_ts {
                overridden_ts.entry(ev.uid.clone()).or_default().insert(ts);
            }
        }

        for ev in &parsed {
            if ev.is_all_day {
                continue;
            }

            if let Some(rrule_val) = &ev.rrule {
                let overrides = overridden_ts.get(&ev.uid);
                let occurrences = expand_rrule_for_day(
                    &ev.dtstart_val,
                    ev.dtstart_tzid.as_deref(),
                    rrule_val,
                    &ev.exdates,
                    day_naive,
                );
                for dt in occurrences {
                    let ts = dt.timestamp();
                    if overrides.map(|s| s.contains(&ts)).unwrap_or(false) {
                        continue;
                    }
                    let time = dt.format("%H:%M").to_string();
                    let id = format!("calendar:{}:{}", ev.uid, ts);
                    if !seen_ids.insert(id.clone()) {
                        continue;
                    }
                    rows.push((
                        ts,
                        TimelineEvent {
                            id,
                            time,
                            source: TimelineEventSource::Calendar,
                            title: ev.summary.clone(),
                            detail: None,
                            url: ev.url.clone(),
                        },
                    ));
                }
            } else {
                let dt = match parse_ics_datetime(&ev.dtstart_val) {
                    Some(d) => d,
                    None => continue,
                };
                if dt.date_naive() != day_naive {
                    continue;
                }
                let ts = dt.timestamp();
                let time = dt.format("%H:%M").to_string();
                let id = format!("calendar:{}", ev.uid);
                if !seen_ids.insert(id.clone()) {
                    continue;
                }
                rows.push((
                    ts,
                    TimelineEvent {
                        id,
                        time,
                        source: TimelineEventSource::Calendar,
                        title: ev.summary.clone(),
                        detail: None,
                        url: ev.url.clone(),
                    },
                ));
            }
        }
    }

    rows.sort_by_key(|(ts, _)| *ts);
    Ok(rows)
}

fn parse_ics_datetime(value: &str) -> Option<DateTime<Local>> {
    if value.ends_with('Z') {
        let naive =
            NaiveDateTime::parse_from_str(&value[..value.len() - 1], "%Y%m%dT%H%M%S").ok()?;
        Some(Utc.from_utc_datetime(&naive).with_timezone(&Local))
    } else {
        let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
        Local.from_local_datetime(&naive).earliest()
    }
}
