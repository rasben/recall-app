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

/// One day's worth of merged timeline events, used by the export command so the
/// frontend can group events under their day without re-deriving the date from
/// each event's local `HH:MM` time.
#[derive(Serialize, specta::Type)]
pub struct ExportDay {
    /// Local calendar day, `YYYY-MM-DD`.
    pub date: String,
    /// Events on that day, sorted ascending by timestamp.
    pub events: Vec<TimelineEvent>,
}

/// A source that failed during an export, so the frontend can warn that the
/// copied result is incomplete rather than presenting it as the full picture.
#[derive(Serialize, specta::Type)]
pub struct ExportSourceError {
    pub source: String,
    pub error: String,
}

/// Result of an export: the per-day events plus any sources that failed to
/// fetch. `errors` is empty on a fully successful export.
#[derive(Serialize, specta::Type)]
pub struct ExportResult {
    pub days: Vec<ExportDay>,
    pub errors: Vec<ExportSourceError>,
}

/// Maximum span (inclusive, in days) a single export may cover. Guards against
/// an accidental multi-year live fetch hammering third-party APIs.
const MAX_EXPORT_DAYS: i64 = 366;

/// Per-local-day timestamped events, as fetched from the sources.
type DayBuckets = HashMap<NaiveDate, Vec<(i64, TimelineEvent)>>;
/// `(source name, error message)` for each source that failed during a fetch.
type SourceErrors = Vec<(&'static str, String)>;

/// Fetch every timeline source once over the contiguous range spanning `wanted`
/// (its first..=last day) and bucket the results by local day. Only days present
/// in `wanted` get a bucket — events landing on other days within the spanned
/// range are dropped. `emit(source, done, error)` is invoked before (done=false)
/// and after (done=true, with any error) each source so callers can surface
/// per-source progress. Returns the per-day buckets plus the `(source, error)`
/// of every source that failed, so callers can both skip caching partial
/// results and report the gaps to the user.
fn collect_range_events<F: Fn(&'static str, bool, Option<String>)>(
    state: &State<'_, AppState>,
    wanted: &[NaiveDate],
    emit: F,
) -> (DayBuckets, SourceErrors) {
    let mut per_day: HashMap<NaiveDate, Vec<(i64, TimelineEvent)>> = HashMap::new();
    for day in wanted {
        per_day.insert(*day, Vec::new());
    }
    let mut errors: Vec<(&'static str, String)> = Vec::new();
    if wanted.is_empty() {
        return (per_day, errors);
    }
    let fetch_start = *wanted.first().unwrap();
    let fetch_end = *wanted.last().unwrap();

    let mut extend = |rows: Vec<(NaiveDate, i64, TimelineEvent)>| {
        for (day, ts, ev) in rows {
            if let Some(bucket) = per_day.get_mut(&day) {
                bucket.push((ts, ev));
            }
        }
    };
    let fail = |errors: &mut Vec<(&'static str, String)>, source: &'static str, e: String| {
        emit(source, true, Some(e.clone()));
        errors.push((source, e));
    };

    emit("Git", false, None);
    match git::events_for_range(state, fetch_start, fetch_end) {
        Ok(r) => { extend(r); emit("Git", true, None); }
        Err(e) => fail(&mut errors, "Git", e),
    }

    emit("GitHub", false, None);
    match github::events_for_range(state, fetch_start, fetch_end) {
        Ok(r) => { extend(r); emit("GitHub", true, None); }
        Err(e) => fail(&mut errors, "GitHub", e),
    }

    emit("Calendar", false, None);
    match ical::events_for_range(state, fetch_start, fetch_end) {
        Ok(r) => { extend(r); emit("Calendar", true, None); }
        Err(e) => fail(&mut errors, "Calendar", e),
    }

    emit("Jira", false, None);
    match jira::events_for_range(state, fetch_start, fetch_end) {
        Ok(r) => { extend(r); emit("Jira", true, None); }
        Err(e) => fail(&mut errors, "Jira", e),
    }

    emit("Zulip", false, None);
    match zulip::events_for_range(state, fetch_start, fetch_end) {
        Ok(r) => { extend(r); emit("Zulip", true, None); }
        Err(e) => fail(&mut errors, "Zulip", e),
    }

    (per_day, errors)
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

/// All dates from `start` through `end` inclusive, ascending. An inverted
/// range (`end` before `start`) yields just `start`.
fn days_in_range(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let mut days = Vec::new();
    let mut d = start;
    loop {
        days.push(d);
        if d >= end {
            break;
        }
        let Some(next) = d.succ_opt() else { break };
        d = next;
    }
    days
}

/// Collect merged timeline events for every day in `[start, end]` (inclusive,
/// `YYYY-MM-DD`), grouped per day, for the "export for AI" feature. Reuses the
/// per-day cache for elapsed days and does a single live fetch for the rest
/// (today and any uncached past day), re-caching the elapsed ones. Empty days
/// are included so the consumer sees the full coverage of the range.
#[tauri::command]
#[specta::specta]
pub async fn export_timeline_for_range(
    state: State<'_, AppState>,
    start: String,
    end: String,
) -> Result<ExportResult, String> {
    tokio::task::block_in_place(|| {
        let start_day = NaiveDate::parse_from_str(&start, "%Y-%m-%d")
            .map_err(|_| format!("Invalid start date (expected YYYY-MM-DD): {start}"))?;
        let end_day = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
            .map_err(|_| format!("Invalid end date (expected YYYY-MM-DD): {end}"))?;
        if end_day < start_day {
            return Err(format!("End date {end} is before start date {start}"));
        }
        if (end_day - start_day).num_days() + 1 > MAX_EXPORT_DAYS {
            return Err(format!("Range too large (max {MAX_EXPORT_DAYS} days)"));
        }

        let today = Local::now().date_naive();

        let range_days = days_in_range(start_day, end_day);

        // Reuse cached rows for elapsed days; collect the rest for one live
        // fetch. Today and any future day are always fetched live, never cached.
        let mut by_day: HashMap<NaiveDate, Vec<TimelineEvent>> = HashMap::new();
        let mut uncached: Vec<NaiveDate> = Vec::new();
        for &d in &range_days {
            if d < today {
                let iso = d.format("%Y-%m-%d").to_string();
                match cache::get_cached_day(&state, &iso) {
                    Some(cached) => { by_day.insert(d, cached); }
                    None => uncached.push(d),
                }
            } else {
                uncached.push(d);
            }
        }

        // No progress events here: an export is a one-shot copy, and the
        // returned `errors` (not a side-channel event) is what the UI needs.
        let (mut per_day, errors) = collect_range_events(&state, &uncached, |_, _, _| {});
        let any_error = !errors.is_empty();

        for day in &uncached {
            let mut rows = per_day.remove(day).unwrap_or_default();
            rows.sort_by_key(|(ts, _)| *ts);
            let events: Vec<TimelineEvent> = rows.into_iter().map(|(_, ev)| ev).collect();
            if *day < today && !any_error {
                let iso = day.format("%Y-%m-%d").to_string();
                let _ = cache::save_cached_day(&state, &iso, &events);
            }
            by_day.insert(*day, events);
        }

        // Assemble the ordered output for every day in the range, including
        // days with no activity (empty events vec).
        let days: Vec<ExportDay> = range_days
            .into_iter()
            .map(|d| ExportDay {
                date: d.format("%Y-%m-%d").to_string(),
                events: by_day.remove(&d).unwrap_or_default(),
            })
            .collect();

        Ok(ExportResult {
            days,
            errors: errors
                .into_iter()
                .map(|(source, error)| ExportSourceError { source: source.to_string(), error })
                .collect(),
        })
    })
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

#[tauri::command]
#[specta::specta]
pub async fn test_settings_ical(state: State<'_, AppState>) -> Result<(), String> {
    tokio::task::block_in_place(|| ical::test_connection(&state))
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
        for d in days_in_range(first, range_last) {
            let iso = d.format("%Y-%m-%d").to_string();
            if let Some(cached) = cache::get_cached_day(&state, &iso) {
                counts.insert(iso, cached.len() as u32);
            } else {
                uncached.push(d);
            }
        }

        if uncached.is_empty() {
            return Ok(counts);
        }

        // Fetch each source once for the bounding uncached range, bucketing
        // results by local day. Emit per-source progress so the frontend can
        // show a real progress bar on the "load month" button instead of an
        // opaque spinner. Days with no activity still go into the cache (as an
        // empty row) so we don't re-fetch them next time.
        let (mut per_day, errors) = collect_range_events(&state, &uncached, |source, done, error| {
            let _ = app.emit("month:source", SourceProgress { source, done, error });
        });
        let any_error = !errors.is_empty();

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
