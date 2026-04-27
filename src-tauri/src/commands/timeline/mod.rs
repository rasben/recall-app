mod cache;
mod git;
mod github;
pub(crate) mod ical;
mod jira;
mod zulip;

use std::collections::HashMap;

use chrono::{Local, NaiveDate};
use rusqlite::params;
use serde::Serialize;
use tauri::{Emitter, State};

use crate::state::AppState;
use crate::timeline::TimelineEvent;

#[derive(Serialize, Clone)]
struct SourceProgress {
    source: &'static str,
    done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
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
            let _ = app.emit("timeline:source", SourceProgress { source, done: false, error: None });
        };
        let done = |source: &'static str| {
            let _ = app.emit("timeline:source", SourceProgress { source, done: true, error: None });
        };
        let fail = |source: &'static str, err: String| {
            let _ = app.emit("timeline:source", SourceProgress { source, done: true, error: Some(err) });
        };

        let mut rows: Vec<(i64, TimelineEvent)> = Vec::new();
        let mut any_error = false;

        loading("Git");
        match git::events_for_day(&state, &day) {
            Ok(r) => { rows.extend(r); done("Git"); }
            Err(e) => { any_error = true; fail("Git", e); }
        }

        loading("GitHub");
        match github::events_for_day(&state, &day) {
            Ok(r) => { rows.extend(r); done("GitHub"); }
            Err(e) => { any_error = true; fail("GitHub", e); }
        }

        loading("Calendar");
        match ical::events_for_day(&state, &day) {
            Ok(r) => { rows.extend(r); done("Calendar"); }
            Err(e) => { any_error = true; fail("Calendar", e); }
        }

        loading("Jira");
        match jira::events_for_day(&state, &day) {
            Ok(r) => { rows.extend(r); done("Jira"); }
            Err(e) => { any_error = true; fail("Jira", e); }
        }

        loading("Zulip");
        match zulip::events_for_day(&state, &day) {
            Ok(r) => { rows.extend(r); done("Zulip"); }
            Err(e) => { any_error = true; fail("Zulip", e); }
        }

        rows.sort_by_key(|(ts, _)| *ts);
        let events: Vec<TimelineEvent> = rows.into_iter().map(|(_, ev)| ev).collect();

        // Skip caching if any source failed — partial results must not become
        // permanent since the missing data would never be re-fetched.
        if use_cache && !any_error {
            let _ = cache::save_cached_day(&state, &day, &events);
        }

        Ok(events)
    })
}

/// Drop the cached row for `day` (if any) and re-run the live fetch via
/// `get_timeline_for_day`. Past days that re-fetch successfully will be
/// re-cached by the existing flow; today is fetched live and not cached.
#[tauri::command]
#[specta::specta]
pub async fn refresh_timeline_for_day(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    day: String,
) -> Result<Vec<TimelineEvent>, String> {
    tokio::task::block_in_place(|| {
        let conn = state.db.lock().map_err(|_| "Failed to access database")?;
        conn.execute(
            "DELETE FROM timeline_day_cache WHERE day = ?1",
            params![&day],
        )
        .map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    })?;
    get_timeline_for_day(app, state, day).await
}

#[tauri::command]
#[specta::specta]
pub async fn test_settings_git(state: State<'_, AppState>) -> Result<(), String> {
    tokio::task::block_in_place(|| git::test_connection(&state))
}

#[tauri::command]
#[specta::specta]
pub async fn test_settings_github(state: State<'_, AppState>) -> Result<(), String> {
    tokio::task::block_in_place(|| github::test_connection(&state))
}

#[tauri::command]
#[specta::specta]
pub async fn test_settings_jira(state: State<'_, AppState>) -> Result<(), String> {
    tokio::task::block_in_place(|| jira::test_connection(&state))
}

#[tauri::command]
#[specta::specta]
pub async fn test_settings_zulip(state: State<'_, AppState>) -> Result<(), String> {
    tokio::task::block_in_place(|| zulip::test_connection(&state))
}

/// Fetch event counts for every elapsed day of the given calendar month,
/// populating the per-day cache along the way. Uses one range query per
/// source instead of N per-day queries, so a fresh month completes in a
/// fraction of the time and without hammering third-party rate limits.
#[tauri::command]
#[specta::specta]
pub async fn get_day_counts_for_month(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    year: i32,
    month: u32,
) -> Result<HashMap<String, u32>, String> {
    tokio::task::block_in_place(|| {
        let first =
            NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| format!("Invalid year/month: {year}-{month}"))?;
        let today = Local::now().date_naive();

        // Last day in the month, capped to yesterday — today and future days
        // are never cached and aren't relevant for the heatmap.
        let next_month_first = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1)
        }
        .ok_or_else(|| format!("Invalid month boundary: {year}-{month}"))?;
        let month_last = next_month_first
            .pred_opt()
            .ok_or("No day before next-month boundary")?;
        let yesterday = match today.pred_opt() {
            Some(d) => d,
            None => return Ok(HashMap::new()),
        };
        let range_last = month_last.min(yesterday);
        if first > range_last {
            return Ok(HashMap::new());
        }

        let mut counts: HashMap<String, u32> = HashMap::new();

        // Collect the uncached days — everything already in the cache keeps
        // whatever count it has without triggering any network work.
        let mut uncached: Vec<NaiveDate> = Vec::new();
        let mut d = first;
        loop {
            let iso = d.format("%Y-%m-%d").to_string();
            if let Some(cached) = cache::get_cached_day(&state, &iso) {
                counts.insert(iso, cached.len() as u32);
            } else {
                uncached.push(d);
            }
            if d == range_last {
                break;
            }
            let Some(next) = d.succ_opt() else { break };
            d = next;
        }

        if uncached.is_empty() {
            return Ok(counts);
        }

        // Fetch each source once for the bounding uncached range, then bucket
        // results by local day. Days with no activity still go into the cache
        // (as an empty row) so we don't re-fetch them next time.
        let fetch_start = *uncached.first().unwrap();
        let fetch_end = *uncached.last().unwrap();

        let mut per_day: HashMap<NaiveDate, Vec<(i64, TimelineEvent)>> = HashMap::new();
        for day in &uncached {
            per_day.insert(*day, Vec::new());
        }

        let mut extend = |rows: Vec<(NaiveDate, i64, TimelineEvent)>| {
            for (day, ts, ev) in rows {
                if let Some(bucket) = per_day.get_mut(&day) {
                    bucket.push((ts, ev));
                }
            }
        };

        // Emit per-source progress so the frontend can show a real progress
        // bar on the "load month" button instead of an opaque spinner.
        let loading = |source: &'static str| {
            let _ = app.emit("month:source", SourceProgress { source, done: false, error: None });
        };
        let done = |source: &'static str| {
            let _ = app.emit("month:source", SourceProgress { source, done: true, error: None });
        };
        let fail = |source: &'static str, err: String| {
            let _ = app.emit("month:source", SourceProgress { source, done: true, error: Some(err) });
        };

        let mut any_error = false;

        loading("Git");
        match git::events_for_range(&state, fetch_start, fetch_end) {
            Ok(r) => { extend(r); done("Git"); }
            Err(e) => { any_error = true; fail("Git", e); }
        }

        loading("GitHub");
        match github::events_for_range(&state, fetch_start, fetch_end) {
            Ok(r) => { extend(r); done("GitHub"); }
            Err(e) => { any_error = true; fail("GitHub", e); }
        }

        loading("Calendar");
        match ical::events_for_range(&state, fetch_start, fetch_end) {
            Ok(r) => { extend(r); done("Calendar"); }
            Err(e) => { any_error = true; fail("Calendar", e); }
        }

        loading("Jira");
        match jira::events_for_range(&state, fetch_start, fetch_end) {
            Ok(r) => { extend(r); done("Jira"); }
            Err(e) => { any_error = true; fail("Jira", e); }
        }

        loading("Zulip");
        match zulip::events_for_range(&state, fetch_start, fetch_end) {
            Ok(r) => { extend(r); done("Zulip"); }
            Err(e) => { any_error = true; fail("Zulip", e); }
        }

        for day in uncached {
            let iso = day.format("%Y-%m-%d").to_string();
            let mut rows = per_day.remove(&day).unwrap_or_default();
            rows.sort_by_key(|(ts, _)| *ts);
            let events: Vec<TimelineEvent> = rows.into_iter().map(|(_, ev)| ev).collect();
            counts.insert(iso.clone(), events.len() as u32);
            if !any_error {
                let _ = cache::save_cached_day(&state, &iso, &events);
            }
        }

        Ok(counts)
    })
}
