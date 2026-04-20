mod cache;
mod git;
mod github;
pub(crate) mod ical;
mod jira;
mod zulip;

use chrono::{Local, NaiveDate};
use serde::Serialize;
use tauri::{Emitter, State};

use crate::state::AppState;
use crate::timeline::TimelineEvent;

#[derive(Serialize, Clone)]
struct SourceProgress {
    source: &'static str,
    done: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn get_timeline_for_day(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    day: String,
) -> Result<Vec<TimelineEvent>, String> {
    // block_in_place lets us run sync/blocking code (HTTP, shell, SQLite) without
    // blocking the async executor's thread, preventing the beach-ball freeze.
    tokio::task::block_in_place(|| {
        let day_naive = NaiveDate::parse_from_str(&day, "%Y-%m-%d")
            .map_err(|_| format!("Invalid date (expected YYYY-MM-DD): {day}"))?;
        let today = Local::now().date_naive();

        // Only cache fully-elapsed days.
        let use_cache = day_naive < today;

        if use_cache {
            if let Some(cached) = cache::get_cached_day(&state, &day) {
                return Ok(cached);
            }
        }

        let loading = |source: &'static str| {
            let _ = app.emit("timeline:source", SourceProgress { source, done: false });
        };
        let done = |source: &'static str| {
            let _ = app.emit("timeline:source", SourceProgress { source, done: true });
        };

        let mut rows: Vec<(i64, TimelineEvent)> = Vec::new();

        loading("Git");
        rows.extend(git::events_for_day(&state, &day)?);
        done("Git");

        loading("GitHub");
        rows.extend(github::events_for_day(&state, &day)?);
        done("GitHub");

        loading("Calendar");
        if let Ok(cal_rows) = ical::events_for_day(&state, &day) {
            rows.extend(cal_rows);
        }
        done("Calendar");

        loading("Jira");
        rows.extend(jira::events_for_day(&state, &day)?);
        done("Jira");

        loading("Zulip");
        rows.extend(zulip::events_for_day(&state, &day)?);
        done("Zulip");

        rows.sort_by_key(|(ts, _)| *ts);
        let events: Vec<TimelineEvent> = rows.into_iter().map(|(_, ev)| ev).collect();

        if use_cache {
            let _ = cache::save_cached_day(&state, &day, &events);
        }

        Ok(events)
    })
}
