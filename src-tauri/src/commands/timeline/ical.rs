use chrono::{Local, NaiveDate, TimeZone};
use rusqlite::params;
use tauri::State;

use crate::commands::settings_ical::load_settings_ical;
use crate::state::AppState;
use crate::timeline::{sanitize_event_url, TimelineEvent, TimelineEventSource};

pub(super) fn events_for_day(
    state: &State<'_, AppState>,
    day: &str,
) -> Result<Vec<(i64, TimelineEvent)>, String> {
    let day_naive =
        NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|_| format!("Invalid date: {day}"))?;
    let rows = events_for_range(state, day_naive, day_naive)?;
    Ok(rows
        .into_iter()
        .filter(|(d, _, _)| *d == day_naive)
        .map(|(_, ts, ev)| (ts, ev))
        .collect())
}

pub(super) fn events_for_range(
    state: &State<'_, AppState>,
    start_day: NaiveDate,
    end_day: NaiveDate,
) -> Result<Vec<(NaiveDate, i64, TimelineEvent)>, String> {
    let settings = load_settings_ical(state);
    if !settings.enabled || settings.urls.is_empty() {
        return Ok(Vec::new());
    }

    let next_end = end_day
        .succ_opt()
        .ok_or_else(|| format!("no day after {end_day}"))?;
    let range_start = Local
        .from_local_datetime(&start_day.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .map(|dt| dt.timestamp())
        .ok_or("Invalid range start")?;
    // Exclusive upper bound: midnight after end_day.
    let range_end = Local
        .from_local_datetime(&next_end.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .map(|dt| dt.timestamp())
        .ok_or("Invalid range end")?;

    let conn = state.db.lock().map_err(|_| "DB lock failed".to_string())?;
    // Match events that start inside the range OR start earlier and run into
    // it (overnight events like "On-call Mon 18:00 → Tue 09:00" need to show
    // on every day they cover).
    let mut stmt = conn
        .prepare(
            "SELECT uid, dtstart, dtend, summary, event_url
             FROM ical_events
             WHERE (declined IS NULL OR declined = 0)
               AND (
                 (dtstart >= ?1 AND dtstart < ?2)
                 OR (dtend IS NOT NULL AND dtstart < ?1 AND dtend > ?1)
               )",
        )
        .map_err(|e| e.to_string())?;

    type IcalRow = (String, i64, Option<i64>, String, Option<String>);
    let raw: Vec<IcalRow> = stmt
        .query_map(params![range_start, range_end], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    drop(conn);

    let mut results: Vec<(NaiveDate, i64, TimelineEvent)> = Vec::new();
    for (uid, dtstart, dtend, summary, event_url) in raw {
        let detail = dtend.and_then(|end| {
            let mins = (end - dtstart) / 60;
            if mins <= 0 {
                return None;
            }
            let h = mins / 60;
            let m = mins % 60;
            Some(match (h, m) {
                (0, m) => format!("{m}m"),
                (h, 0) => format!("{h}h"),
                (h, m) => format!("{h}h {m}m"),
            })
        });

        use chrono::DateTime;
        let start_dt = DateTime::from_timestamp(dtstart, 0)
            .map(|d| d.with_timezone(&Local))
            .unwrap_or_else(chrono::Local::now);
        let start_day_local = start_dt.date_naive();

        // Enumerate every local day the event covers, clamped to [start_day, end_day].
        let effective_end_ts = dtend.unwrap_or(dtstart);
        let effective_end_dt = DateTime::from_timestamp(effective_end_ts.max(dtstart), 0)
            .map(|d| d.with_timezone(&Local))
            .unwrap_or(start_dt);
        let mut cover_end_day = effective_end_dt.date_naive();
        // An event ending exactly at midnight doesn't cover the next day.
        if dtend.is_some() && effective_end_dt.time() == chrono::NaiveTime::MIN && cover_end_day > start_day_local {
            cover_end_day = cover_end_day.pred_opt().unwrap_or(cover_end_day);
        }

        let first = start_day_local.max(start_day);
        let last = cover_end_day.min(end_day);
        if first > last {
            continue;
        }

        let mut d = first;
        loop {
            // For overnight events carrying into a day, clamp displayed
            // time to "00:00"; the original dtstart remains the sort key
            // so these events appear at the top of that day.
            let time = if d == start_day_local {
                start_dt.format("%H:%M").to_string()
            } else {
                "00:00".to_string()
            };
            results.push((
                d,
                dtstart,
                TimelineEvent {
                    id: format!("calendar:{uid}"),
                    time,
                    source: TimelineEventSource::Calendar,
                    title: summary.clone(),
                    detail: detail.clone(),
                    url: event_url.as_deref().and_then(sanitize_event_url),
                },
            ));
            if d == last {
                break;
            }
            let Some(next) = d.succ_opt() else { break };
            d = next;
        }
    }

    results.sort_by_key(|(_, ts, _)| *ts);
    Ok(results)
}
