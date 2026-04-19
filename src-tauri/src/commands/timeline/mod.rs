mod cache;
mod git;
mod github;
mod jira;
mod zulip;

use chrono::{Local, NaiveDate};
use tauri::State;

use crate::state::AppState;
use crate::timeline::TimelineEvent;

#[tauri::command]
#[specta::specta]
pub fn get_timeline_for_day(
    state: State<'_, AppState>,
    day: String,
) -> Result<Vec<TimelineEvent>, String> {
    let day_naive = NaiveDate::parse_from_str(&day, "%Y-%m-%d")
        .map_err(|_| format!("Invalid date (expected YYYY-MM-DD): {day}"))?;
    let today = Local::now().date_naive();

    // Only cache fully-elapsed days — today's timeline is still evolving, and
    // future days are never "loaded" in the cache-worthy sense.
    let use_cache = day_naive < today;

    if use_cache {
        if let Some(cached) = cache::get_cached_day(&state, &day) {
            return Ok(cached);
        }
    }

    let mut rows: Vec<(i64, TimelineEvent)> = Vec::new();
    rows.extend(git::events_for_day(&state, &day)?);
    rows.extend(github::events_for_day(&state, &day)?);
    rows.extend(jira::events_for_day(&state, &day)?);
    rows.extend(zulip::events_for_day(&state, &day)?);
    rows.sort_by_key(|(ts, _)| *ts);
    let events: Vec<TimelineEvent> = rows.into_iter().map(|(_, ev)| ev).collect();

    if use_cache {
        // Best-effort: a cache write failure should not break the live fetch.
        let _ = cache::save_cached_day(&state, &day, &events);
    }

    Ok(events)
}
