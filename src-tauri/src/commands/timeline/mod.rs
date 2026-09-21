mod cache;
mod git;
mod github;
pub(crate) mod ical;
mod jira;
mod zulip;

use std::collections::{HashMap, HashSet};

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
/// Days for which some source returned successfully but could not deliver
/// complete data (an API horizon or result cap). Like errors, these block
/// caching — a partial day cached as truth would never be re-fetched.
type IncompleteDays = HashSet<NaiveDate>;

/// Fetch every timeline source once over the contiguous range spanning `wanted`
/// (its first..=last day) and bucket the results by local day. Only days present
/// in `wanted` get a bucket — events landing on other days within the spanned
/// range are dropped. `emit(source, done, error)` is invoked before (done=false)
/// and after (done=true, with any error) each source so callers can surface
/// per-source progress. Returns the per-day buckets plus the `(source, error)`
/// of every source that failed, so callers can both skip caching partial
/// results and report the gaps to the user, plus the days any source reported
/// as incomplete (also not to be cached).
fn collect_range_events<F: Fn(&'static str, bool, Option<String>)>(
    state: &State<'_, AppState>,
    wanted: &[NaiveDate],
    emit: F,
) -> (DayBuckets, SourceErrors, IncompleteDays) {
    let mut per_day: HashMap<NaiveDate, Vec<(i64, TimelineEvent)>> = HashMap::new();
    for day in wanted {
        per_day.insert(*day, Vec::new());
    }
    let mut errors: Vec<(&'static str, String)> = Vec::new();
    let mut incomplete: IncompleteDays = HashSet::new();
    if wanted.is_empty() {
        return (per_day, errors, incomplete);
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
        Ok((r, partial)) => { extend(r); incomplete.extend(partial); emit("GitHub", true, None); }
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

    (per_day, errors, incomplete)
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
        let mut any_incomplete = false;

        loading("Git");
        match git::events_for_day(&state, &day) {
            Ok(r) => { rows.extend(r); done("Git"); }
            Err(e) => { any_error = true; fail("Git", e); }
        }

        loading("GitHub");
        match github::events_for_day(&state, &day) {
            Ok((r, partial)) => { rows.extend(r); any_incomplete |= partial; done("GitHub"); }
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

        // Skip caching if any source failed or reported incomplete data —
        // partial results must not become permanent since the missing data
        // would never be re-fetched.
        if use_cache && !any_error && !any_incomplete {
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
        let (mut per_day, errors, incomplete) = collect_range_events(&state, &uncached, |_, _, _| {});
        let any_error = !errors.is_empty();

        for day in &uncached {
            let mut rows = per_day.remove(day).unwrap_or_default();
            rows.sort_by_key(|(ts, _)| *ts);
            let events: Vec<TimelineEvent> = rows.into_iter().map(|(_, ev)| ev).collect();
            if *day < today && !any_error && !incomplete.contains(day) {
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
        let (mut per_day, errors, incomplete) = collect_range_events(&state, &uncached, |source, done, error| {
            let _ = app.emit("month:source", SourceProgress { source, done, error });
        });
        let any_error = !errors.is_empty();

        for day in uncached {
            let iso = day.format("%Y-%m-%d").to_string();
            let mut rows = per_day.remove(&day).unwrap_or_default();
            rows.sort_by_key(|(ts, _)| *ts);
            let events: Vec<TimelineEvent> = rows.into_iter().map(|(_, ev)| ev).collect();
            counts.insert(iso.clone(), events.len() as u32);
            if !any_error && !incomplete.contains(&day) {
                let _ = cache::save_cached_day(&state, &iso, &events);
            }
        }

        Ok(counts)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use crate::commands::settings_git::{set_settings_git, SettingsGit};
    use crate::commands::settings_github::{set_settings_github, GitHubEvent, SettingsGitHub};
    use crate::commands::settings_jira::{set_settings_jira, JiraEvent, SettingsJira};
    use crate::commands::settings_zulip::{set_settings_zulip, SettingsZulip};
    use crate::test_support::{event, local_ts, mock_app, runtime, seed_setting, state};
    use crate::timeline::TimelineEventSource;

    type MockApp = tauri::App<tauri::test::MockRuntime>;
    type Emitted = (&'static str, bool, Option<String>);

    const SOURCES: [&str; 5] = ["Git", "GitHub", "Calendar", "Jira", "Zulip"];

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn iso(d: NaiveDate) -> String {
        d.format("%Y-%m-%d").to_string()
    }

    /// Point the Git source at a directory that does not exist: a
    /// deterministic, network-free way to make one source fail.
    fn break_git(state: &State<'_, AppState>) {
        let path = std::env::temp_dir().join(format!("recall-missing-{}", uuid::Uuid::new_v4()));
        seed_setting(
            state,
            "settings_git",
            &format!(r#"{{"enabled":true,"path":{}}}"#, serde_json::to_string(&path).unwrap()),
        );
    }

    fn enable_calendar(state: &State<'_, AppState>) {
        seed_setting(
            state,
            crate::commands::settings_ical::KEY,
            r#"{"enabled":true,"urls":["https://cal.example/feed.ics"],"emails":[]}"#,
        );
    }

    fn insert_calendar_event(state: &State<'_, AppState>, uid: &str, day: NaiveDate, hh: u32, mm: u32) {
        let conn = state.db.lock().unwrap();
        conn.execute(
            "INSERT INTO ical_events (url, uid, dtstart, dtend, summary, event_url, declined)
             VALUES ('https://cal.example/feed.ics', ?1, ?2, NULL, ?3, NULL, 0)",
            params![uid, local_ts(day, hh, mm), format!("Event {uid}")],
        )
        .unwrap();
    }

    fn collect(app: &MockApp, wanted: &[NaiveDate]) -> (DayBuckets, SourceErrors, Vec<Emitted>) {
        let emitted: Mutex<Vec<Emitted>> = Mutex::new(Vec::new());
        let (per_day, errors) = collect_range_events(&state(app), wanted, |source, done, error| {
            emitted.lock().unwrap().push((source, done, error));
        });
        (per_day, errors, emitted.into_inner().unwrap())
    }

    fn export(app: &MockApp, start: &str, end: &str) -> Result<ExportResult, String> {
        runtime().block_on(export_timeline_for_range(state(app), start.to_string(), end.to_string()))
    }

    fn cached_days(state: &State<'_, AppState>) -> Vec<String> {
        let conn = state.db.lock().unwrap();
        let mut stmt = conn.prepare("SELECT day FROM timeline_day_cache ORDER BY day").unwrap();
        stmt.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
    }

    // ── days_in_range ───────────────────────────────────────────────────────

    #[test]
    fn days_in_range_single_day() {
        assert_eq!(days_in_range(d("2024-03-05"), d("2024-03-05")), vec![d("2024-03-05")]);
    }

    #[test]
    fn days_in_range_is_inclusive_ascending_and_crosses_month_ends() {
        let days = days_in_range(d("2024-02-27"), d("2024-03-02"));
        assert_eq!(
            days.iter().map(|x| iso(*x)).collect::<Vec<_>>(),
            vec!["2024-02-27", "2024-02-28", "2024-02-29", "2024-03-01", "2024-03-02"]
        );
    }

    #[test]
    fn days_in_range_inverted_yields_only_the_start() {
        assert_eq!(days_in_range(d("2024-03-05"), d("2024-03-01")), vec![d("2024-03-05")]);
    }

    // ── collect_range_events ────────────────────────────────────────────────

    #[test]
    fn collect_with_no_wanted_days_does_nothing() {
        let app = mock_app();
        let (per_day, errors, emitted) = collect(&app, &[]);
        assert!(per_day.is_empty());
        assert!(errors.is_empty());
        assert!(emitted.is_empty(), "no source should be touched for an empty range");
    }

    #[test]
    fn collect_with_no_sources_configured_yields_an_empty_bucket_per_wanted_day() {
        let app = mock_app();
        let wanted = [d("2024-03-05"), d("2024-03-07")];
        let (per_day, errors, _) = collect(&app, &wanted);
        assert!(errors.is_empty());
        let mut keys: Vec<_> = per_day.keys().copied().collect();
        keys.sort();
        assert_eq!(keys, wanted.to_vec(), "only wanted days get a bucket, gaps do not");
        assert!(per_day.values().all(|v| v.is_empty()));
    }

    #[test]
    fn collect_emits_loading_then_done_for_every_source() {
        let app = mock_app();
        let (_, _, emitted) = collect(&app, &[d("2024-03-05")]);
        assert_eq!(emitted.len(), SOURCES.len() * 2);
        for source in SOURCES {
            let loading = emitted.iter().position(|(s, done, _)| *s == source && !done);
            let done = emitted.iter().position(|(s, done, _)| *s == source && *done);
            let (loading, done) = (
                loading.unwrap_or_else(|| panic!("{source} never reported loading")),
                done.unwrap_or_else(|| panic!("{source} never reported done")),
            );
            assert!(loading < done, "{source}: loading must precede done");
            assert_eq!(emitted[done].2, None, "{source}: no error expected");
        }
    }

    #[test]
    fn collect_reports_a_failing_source_and_keeps_the_others() {
        let app = mock_app();
        break_git(&state(&app));
        let (per_day, errors, emitted) = collect(&app, &[d("2024-03-05")]);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].0, "Git");
        assert!(!errors[0].1.is_empty(), "the error message is passed through");
        assert!(per_day[&d("2024-03-05")].is_empty());

        let git_done = emitted.iter().find(|(s, done, _)| *s == "Git" && *done).unwrap();
        assert_eq!(git_done.2.as_deref(), Some(errors[0].1.as_str()));
        for source in SOURCES.iter().filter(|s| **s != "Git") {
            let done = emitted.iter().find(|(s, done, _)| s == source && *done).unwrap();
            assert_eq!(done.2, None, "{source} should still succeed");
        }
    }

    #[test]
    fn collect_buckets_calendar_events_by_local_day_and_drops_unwanted_days() {
        let app = mock_app();
        let s = state(&app);
        enable_calendar(&s);
        insert_calendar_event(&s, "a", d("2024-03-05"), 10, 0);
        insert_calendar_event(&s, "b", d("2024-03-06"), 11, 0); // inside the span, not wanted
        insert_calendar_event(&s, "c", d("2024-03-07"), 12, 0);
        insert_calendar_event(&s, "d", d("2024-03-08"), 9, 0); // outside the span

        let (per_day, errors, _) = collect(&app, &[d("2024-03-05"), d("2024-03-07")]);
        assert!(errors.is_empty());
        assert_eq!(per_day.len(), 2);
        let ids = |day: &str| -> Vec<String> {
            per_day[&d(day)].iter().map(|(_, ev)| ev.id.clone()).collect()
        };
        assert_eq!(ids("2024-03-05"), vec!["calendar:a"]);
        assert_eq!(ids("2024-03-07"), vec!["calendar:c"]);
        assert_eq!(per_day[&d("2024-03-05")][0].0, local_ts(d("2024-03-05"), 10, 0));
        assert_eq!(per_day[&d("2024-03-05")][0].1.source, TimelineEventSource::Calendar);
    }

    // ── sources without configuration are no-ops (and never touch the network) ──

    #[test]
    fn every_source_returns_empty_when_it_has_no_settings() {
        let app = mock_app();
        let s = state(&app);
        let (a, b) = (d("2024-03-05"), d("2024-03-06"));
        assert_eq!(git::events_for_range(&s, a, b).unwrap().len(), 0);
        assert_eq!(github::events_for_range(&s, a, b).unwrap().len(), 0);
        assert_eq!(ical::events_for_range(&s, a, b).unwrap().len(), 0);
        assert_eq!(jira::events_for_range(&s, a, b).unwrap().len(), 0);
        assert_eq!(zulip::events_for_range(&s, a, b).unwrap().len(), 0);
    }

    #[test]
    fn disabled_sources_return_empty_even_with_credentials() {
        // Typed setters, not seeded JSON: a struct shape change must fail to
        // compile here rather than silently make these documents unparseable
        // (which would also return "empty", for the wrong reason).
        let app = mock_app();
        let s = state(&app);
        set_settings_git(s.clone(), SettingsGit { enabled: false, path: "/".into() }).unwrap();
        set_settings_github(
            s.clone(),
            SettingsGitHub {
                enabled: false,
                username: "octocat".into(),
                token: "ghp_x".into(),
                enabled_events: vec![GitHubEvent::PullRequestEvent],
            },
        )
        .unwrap();
        set_settings_jira(
            s.clone(),
            SettingsJira {
                enabled: false,
                site_url: "https://x.atlassian.net".into(),
                email: "a@b.c".into(),
                api_token: "t".into(),
                enabled_events: vec![JiraEvent::CommentWritten],
            },
        )
        .unwrap();
        set_settings_zulip(
            s.clone(),
            SettingsZulip {
                enabled: false,
                realm_url: "https://x.zulipchat.com".into(),
                email: "a@b.c".into(),
                api_key: "k".into(),
            },
        )
        .unwrap();
        let (a, b) = (d("2024-03-05"), d("2024-03-06"));
        assert!(git::events_for_range(&s, a, b).unwrap().is_empty());
        assert!(github::events_for_range(&s, a, b).unwrap().is_empty());
        assert!(jira::events_for_range(&s, a, b).unwrap().is_empty());
        assert!(zulip::events_for_range(&s, a, b).unwrap().is_empty());
    }

    #[test]
    fn enabled_sources_with_incomplete_credentials_return_empty() {
        let app = mock_app();
        let s = state(&app);
        let (a, b) = (d("2024-03-05"), d("2024-03-06"));

        set_settings_git(s.clone(), SettingsGit { enabled: true, path: String::new() }).unwrap();
        assert!(git::events_for_range(&s, a, b).unwrap().is_empty());

        let github = |token: &str, events: Vec<GitHubEvent>| SettingsGitHub {
            enabled: true,
            username: "octocat".into(),
            token: token.into(),
            enabled_events: events,
        };
        set_settings_github(s.clone(), github("", vec![GitHubEvent::PullRequestEvent])).unwrap();
        assert!(github::events_for_range(&s, a, b).unwrap().is_empty());
        set_settings_github(s.clone(), github("ghp_x", vec![])).unwrap();
        assert!(github::events_for_range(&s, a, b).unwrap().is_empty(), "no event types opted in");

        let jira = |site_url: &str, email: &str| SettingsJira {
            enabled: true,
            site_url: site_url.into(),
            email: email.into(),
            api_token: "t".into(),
            enabled_events: vec![JiraEvent::CommentWritten],
        };
        set_settings_jira(s.clone(), jira("https://x.atlassian.net", "  ")).unwrap();
        assert!(jira::events_for_range(&s, a, b).unwrap().is_empty());
        set_settings_jira(s.clone(), jira("", "a@b.c")).unwrap();
        assert!(jira::events_for_range(&s, a, b).unwrap().is_empty());

        set_settings_zulip(
            s.clone(),
            SettingsZulip {
                enabled: true,
                realm_url: "https://x.zulipchat.com".into(),
                email: "a@b.c".into(),
                api_key: String::new(),
            },
        )
        .unwrap();
        assert!(zulip::events_for_range(&s, a, b).unwrap().is_empty());
    }

    // ── export_timeline_for_range ───────────────────────────────────────────

    #[test]
    fn export_rejects_malformed_dates() {
        let app = mock_app();
        assert!(export(&app, "2024-13-01", "2024-03-05").is_err());
        assert!(export(&app, "2024-03-05", "05/03/2024").is_err());
    }

    #[test]
    fn export_rejects_an_inverted_range() {
        let app = mock_app();
        assert!(export(&app, "2024-03-06", "2024-03-05").is_err());
    }

    #[test]
    fn export_enforces_the_maximum_span() {
        let app = mock_app();
        // 2020-01-01 ..= 2020-12-31 is 366 days (leap year): allowed.
        let ok = export(&app, "2020-01-01", "2020-12-31").unwrap();
        assert_eq!(ok.days.len(), 366);
        // One more day is not.
        assert!(export(&app, "2020-01-01", "2021-01-01").is_err());
    }

    #[test]
    fn export_covers_every_day_in_order_including_empty_ones() {
        let app = mock_app();
        let result = export(&app, "2020-02-27", "2020-03-02").unwrap();
        assert!(result.errors.is_empty());
        assert_eq!(
            result.days.iter().map(|x| x.date.as_str()).collect::<Vec<_>>(),
            vec!["2020-02-27", "2020-02-28", "2020-02-29", "2020-03-01", "2020-03-02"]
        );
        assert!(result.days.iter().all(|x| x.events.is_empty()));
    }

    #[test]
    fn export_returns_cached_events_verbatim_for_elapsed_days() {
        let app = mock_app();
        let s = state(&app);
        // Cached out of order on purpose: the cache is trusted as-is.
        let late = event("git:/r:late", local_ts(d("2020-05-05"), 16, 0), TimelineEventSource::Git);
        let early = event("jira:X-1:2020-05-05:Commented", local_ts(d("2020-05-05"), 9, 0), TimelineEventSource::Jira);
        cache::save_cached_day(&s, "2020-05-05", &[late.clone(), early.clone()]).unwrap();

        let result = export(&app, "2020-05-04", "2020-05-06").unwrap();
        assert!(result.errors.is_empty());
        assert_eq!(result.days.len(), 3);
        assert!(result.days[0].events.is_empty());
        assert_eq!(
            result.days[1].events.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec![late.id.as_str(), early.id.as_str()]
        );
        assert!(result.days[2].events.is_empty());
    }

    #[test]
    fn export_caches_elapsed_days_but_never_today_or_the_future() {
        let app = mock_app();
        let s = state(&app);
        // The command reads the clock itself, so bracket it: any day that was
        // elapsed before the call must be cached, and nothing that is still
        // today-or-later after the call may be.
        let before = Local::now().date_naive();
        let start = before - chrono::Duration::days(2);
        let end = before + chrono::Duration::days(1);

        let result = export(&app, &iso(start), &iso(end)).unwrap();
        let after = Local::now().date_naive();

        assert_eq!(result.days.len(), 4);
        assert!(result.errors.is_empty());
        let cached = cached_days(&s);
        for day in days_in_range(start, end) {
            let is_cached = cached.contains(&iso(day));
            if day < before {
                assert!(is_cached, "{day} had elapsed before the export and must be cached");
            }
            if day >= after {
                assert!(!is_cached, "{day} is today or later and must not be cached");
            }
        }
    }

    #[test]
    fn export_reports_source_errors_and_does_not_cache_partial_days() {
        let app = mock_app();
        let s = state(&app);
        break_git(&s);

        let result = export(&app, "2020-05-05", "2020-05-06").unwrap();
        assert_eq!(result.days.len(), 2, "the export still covers every day");
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].source, "Git");
        assert!(!result.errors[0].error.is_empty());
        assert!(cached_days(&s).is_empty(), "a failed source must not poison the cache");
    }

    #[test]
    fn export_merges_live_calendar_events_sorted_and_caches_them() {
        let app = mock_app();
        let s = state(&app);
        enable_calendar(&s);
        insert_calendar_event(&s, "pm", d("2020-05-05"), 14, 0);
        insert_calendar_event(&s, "am", d("2020-05-05"), 9, 0);
        insert_calendar_event(&s, "next", d("2020-05-06"), 10, 0);

        let first = export(&app, "2020-05-05", "2020-05-06").unwrap();
        assert!(first.errors.is_empty());
        assert_eq!(
            first.days[0].events.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["calendar:am", "calendar:pm"]
        );
        assert_eq!(first.days[0].events[0].time, "09:00");
        assert_eq!(first.days[1].events.len(), 1);
        assert_eq!(cached_days(&s), vec!["2020-05-05", "2020-05-06"]);

        // Wipe the live table: a second export must be served from the cache.
        s.db.lock().unwrap().execute("DELETE FROM ical_events", []).unwrap();
        let second = export(&app, "2020-05-05", "2020-05-06").unwrap();
        assert_eq!(
            second.days[0].events.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["calendar:am", "calendar:pm"]
        );
        assert_eq!(second.days[1].events.len(), 1);
    }
}
