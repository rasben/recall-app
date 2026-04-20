use chrono::{Local, NaiveDate, TimeZone};
use rusqlite::params;
use tauri::State;

use crate::commands::settings_ical::load_settings_ical;
use crate::state::AppState;
use crate::timeline::{TimelineEvent, TimelineEventSource};

pub(super) fn events_for_day(
    state: &State<'_, AppState>,
    day: &str,
) -> Result<Vec<(i64, TimelineEvent)>, String> {
    let settings = load_settings_ical(state);
    if !settings.enabled || settings.urls.is_empty() {
        return Ok(Vec::new());
    }

    let day_naive =
        NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|_| format!("Invalid date: {day}"))?;

    let day_start = Local
        .from_local_datetime(&day_naive.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .map(|dt| dt.timestamp())
        .ok_or("Invalid day start")?;
    let day_end = Local
        .from_local_datetime(&day_naive.and_hms_opt(23, 59, 59).unwrap())
        .earliest()
        .map(|dt| dt.timestamp())
        .ok_or("Invalid day end")?;

    let conn = state.db.lock().map_err(|_| "DB lock failed".to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT uid, dtstart, dtend, summary, event_url
             FROM ical_events
             WHERE dtstart >= ?1 AND dtstart <= ?2 AND (declined IS NULL OR declined = 0)",
        )
        .map_err(|e| e.to_string())?;

    let active_urls: std::collections::HashSet<String> = settings
        .urls
        .into_iter()
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
        .collect();

    let rows = stmt
        .query_map(params![day_start, day_end], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut results: Vec<(i64, TimelineEvent)> = rows
        .filter_map(|r| r.ok())
        .map(|(uid, dtstart, dtend, summary, event_url)| {
            let time = {
                use chrono::DateTime;
                let dt = DateTime::from_timestamp(dtstart, 0)
                    .map(|d| d.with_timezone(&Local))
                    .unwrap_or_else(chrono::Local::now);
                dt.format("%H:%M").to_string()
            };
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
            (
                dtstart,
                TimelineEvent {
                    id: format!("calendar:{uid}"),
                    time,
                    source: TimelineEventSource::Calendar,
                    title: summary,
                    detail,
                    url: event_url,
                },
            )
        })
        .collect();

    let _ = active_urls; // ical_events is cleared when URLs change, so no extra filtering needed

    results.sort_by_key(|(ts, _)| *ts);
    Ok(results)
}
