use crate::commands::settings::{get_val, now, save_val};
use crate::state::AppState;
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use ical::IcalParser;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{HashMap, HashSet};
use std::io::BufReader;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::State;

pub(crate) const KEY: &str = "settings_ical";

#[derive(Deserialize, Serialize, Type, Default)]
#[specta(export = false)]
pub struct SettingsIcal {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub urls: Vec<String>,
}

pub(crate) fn load_settings_ical(state: &State<'_, AppState>) -> SettingsIcal {
    get_val(state, KEY)
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default()
}

#[tauri::command]
#[specta::specta]
pub fn get_settings_ical(state: State<'_, AppState>) -> Option<SettingsIcal> {
    get_val(&state, KEY).and_then(|j| serde_json::from_str(&j).ok())
}

#[tauri::command]
#[specta::specta]
pub fn set_settings_ical(
    state: State<'_, AppState>,
    settings: SettingsIcal,
) -> Result<(), String> {
    let old = load_settings_ical(&state);
    let urls_changed = old.urls != settings.urls;
    let should_sync = settings.enabled && (urls_changed || !old.enabled);

    let json = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    save_val(&state, KEY, &json)?;

    if urls_changed {
        let conn = state.db.lock().map_err(|_| "DB lock failed".to_string())?;
        conn.execute("DELETE FROM timeline_day_cache", [])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM ical_events", [])
            .map_err(|e| e.to_string())?;
    }

    if should_sync {
        start_sync(&state);
    }

    Ok(())
}

// ── Sync status ────────────────────────────────────────────────────────────────

#[derive(Serialize, Type, Clone)]
pub struct IcalSyncStatus {
    pub syncing: bool,
    pub last_synced_at: Option<f64>,
    pub last_error: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn get_ical_sync_status(state: State<'_, AppState>) -> IcalSyncStatus {
    let syncing = state.ical_syncing.load(Ordering::Relaxed);
    let conn = match state.db.lock() {
        Ok(c) => c,
        Err(_) => {
            return IcalSyncStatus {
                syncing,
                last_synced_at: None,
                last_error: None,
            }
        }
    };
    let meta: Option<(Option<i64>, Option<String>)> = conn
        .query_row(
            "SELECT last_synced_at, last_error FROM ical_sync_meta WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();
    let (last_synced_at, last_error) = meta.unwrap_or((None, None));
    IcalSyncStatus {
        syncing,
        last_synced_at: last_synced_at.map(|t| t as f64),
        last_error,
    }
}

/// Kicks off a background iCal sync. Returns immediately; poll `get_ical_sync_status` for progress.
#[tauri::command]
#[specta::specta]
pub fn trigger_ical_sync(state: State<'_, AppState>) {
    start_sync(&state);
}

pub(crate) fn start_sync(state: &State<'_, AppState>) {
    if state.ical_syncing.swap(true, Ordering::AcqRel) {
        return; // already running
    }

    let settings = load_settings_ical(state);
    let urls: Vec<String> = settings
        .urls
        .into_iter()
        .filter(|u| !u.trim().is_empty())
        .collect();

    if !settings.enabled || urls.is_empty() {
        state.ical_syncing.store(false, Ordering::Release);
        return;
    }

    // The sync opens its own connection so it never contends with the main DB mutex.
    let db_path = state.db_path.clone();
    let syncing = Arc::clone(&state.ical_syncing);

    tauri::async_runtime::spawn_blocking(move || {
        run_sync(db_path, syncing, urls);
    });
}

fn run_sync(
    db_path: std::path::PathBuf,
    syncing: Arc<AtomicBool>,
    urls: Vec<String>,
) {
    let mut conn = match rusqlite::Connection::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            syncing.store(false, Ordering::Release);
            return;
        }
    };
    // WAL mode must be set on each new connection.
    let _ = conn.execute_batch("PRAGMA journal_mode=WAL;");

    let today = Local::now().date_naive();
    let window_start = today - Duration::days(365 * 3);
    let window_end = today + Duration::days(365);

    let mut last_error: Option<String> = None;

    for url in &urls {
        if let Err(e) = sync_one_url(&mut conn, url.trim(), window_start, window_end) {
            last_error = Some(e);
        }
    }

    let ts = now();
    let _ = conn.execute(
        "INSERT OR REPLACE INTO ical_sync_meta (id, last_synced_at, last_error) VALUES (1, ?1, ?2)",
        params![ts, last_error],
    );
    // Invalidate the day cache so freshly synced events appear.
    let _ = conn.execute("DELETE FROM timeline_day_cache", []);

    syncing.store(false, Ordering::Release);
}

fn sync_one_url(
    conn: &mut rusqlite::Connection,
    url: &str,
    window_start: NaiveDate,
    window_end: NaiveDate,
) -> Result<(), String> {
    let content = ureq::get(url)
        .set("Accept", "text/calendar")
        .call()
        .map_err(|e| format!("iCal fetch: {e}"))?
        .into_string()
        .map_err(|e| format!("iCal read: {e}"))?;

    let events = parse_and_expand(&content, window_start, window_end);

    // A single transaction makes thousands of inserts orders of magnitude faster.
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM ical_events WHERE url = ?1", params![url])
        .map_err(|e| e.to_string())?;
    for ev in &events {
        tx.execute(
            "INSERT OR REPLACE INTO ical_events (url, uid, dtstart, summary, event_url)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![url, ev.uid, ev.dtstart, ev.summary, ev.event_url],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// ── Parsing ────────────────────────────────────────────────────────────────────

struct ParsedEvent {
    uid: String,
    summary: String,
    event_url: Option<String>,
    dtstart_val: String,
    dtstart_tzid: Option<String>,
    is_all_day: bool,
    rrule: Option<String>,
    exdates: Vec<String>,
    recurrence_id_ts: Option<i64>,
}

struct ExpandedEvent {
    uid: String,
    dtstart: i64,
    summary: String,
    event_url: Option<String>,
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
                event_url: get("URL"),
                dtstart_val,
                dtstart_tzid,
                is_all_day,
                rrule: get("RRULE"),
                exdates,
                recurrence_id_ts,
            });
        }
    }

    events
}

fn parse_and_expand(
    body: &str,
    window_start: NaiveDate,
    window_end: NaiveDate,
) -> Vec<ExpandedEvent> {
    let parsed = parse_events_from_ics(body);

    // Collect RECURRENCE-ID overrides per uid so we can skip those master occurrences.
    let mut overridden_ts: HashMap<String, HashSet<i64>> = HashMap::new();
    for ev in &parsed {
        if let Some(ts) = ev.recurrence_id_ts {
            overridden_ts.entry(ev.uid.clone()).or_default().insert(ts);
        }
    }

    let mut results: Vec<ExpandedEvent> = Vec::new();

    for ev in &parsed {
        if ev.is_all_day {
            continue;
        }

        if let Some(rrule_val) = &ev.rrule {
            let overrides = overridden_ts.get(&ev.uid);
            let occurrences = expand_rrule_for_range(
                &ev.dtstart_val,
                ev.dtstart_tzid.as_deref(),
                rrule_val,
                &ev.exdates,
                window_start,
                window_end,
            );
            for dt in occurrences {
                let ts = dt.timestamp();
                if overrides.map(|s| s.contains(&ts)).unwrap_or(false) {
                    continue;
                }
                results.push(ExpandedEvent {
                    uid: format!("{}:{}", ev.uid, ts),
                    dtstart: ts,
                    summary: ev.summary.clone(),
                    event_url: ev.event_url.clone(),
                });
            }
        } else {
            let dt = match parse_ics_datetime(&ev.dtstart_val) {
                Some(d) => d,
                None => continue,
            };
            let date = dt.date_naive();
            if date < window_start || date > window_end {
                continue;
            }
            results.push(ExpandedEvent {
                uid: ev.uid.clone(),
                dtstart: dt.timestamp(),
                summary: ev.summary.clone(),
                event_url: ev.event_url.clone(),
            });
        }
    }

    results
}

fn expand_rrule_for_range(
    dtstart_val: &str,
    dtstart_tzid: Option<&str>,
    rrule_val: &str,
    exdates: &[String],
    window_start: NaiveDate,
    window_end: NaiveDate,
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

    let start_local = match Local
        .from_local_datetime(&window_start.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
    {
        Some(dt) => dt,
        None => return vec![],
    };
    let end_local = match Local
        .from_local_datetime(&window_end.and_hms_opt(23, 59, 59).unwrap())
        .earliest()
    {
        Some(dt) => dt,
        None => return vec![],
    };

    let after_tz: DateTime<rrule::Tz> = start_local.with_timezone(&rrule::Tz::UTC);
    let before_tz: DateTime<rrule::Tz> = end_local.with_timezone(&rrule::Tz::UTC);

    rrule_set
        .after(after_tz)
        .before(before_tz)
        .all(5000)
        .dates
        .into_iter()
        .map(|dt| Utc.from_utc_datetime(&dt.naive_utc()).with_timezone(&Local))
        .collect()
}

pub(crate) fn parse_ics_datetime(value: &str) -> Option<DateTime<Local>> {
    if value.ends_with('Z') {
        let naive =
            NaiveDateTime::parse_from_str(&value[..value.len() - 1], "%Y%m%dT%H%M%S").ok()?;
        Some(Utc.from_utc_datetime(&naive).with_timezone(&Local))
    } else {
        let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
        Local.from_local_datetime(&naive).earliest()
    }
}
