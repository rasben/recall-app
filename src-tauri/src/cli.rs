//! `recall-cli`: headless access to the timeline for scripts and AI agents.
//!
//! It reads the same SQLite database as the desktop app (settings, source
//! credentials, day cache, Harvest checkmarks) and runs the same fetch code,
//! so a caller gets the merged timeline without ever handling a credential.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use serde::Serialize;

use crate::commands::harvest_done::done_event_ids;
use crate::commands::settings::now;
use crate::commands::settings_ical;
use crate::commands::timeline::{export_range, ExportResult, ExportSourceError};
use crate::db;
use crate::state::AppState;
use crate::telemetry;
use crate::timeline::{TimelineEvent, TimelineEventSource};

/// Skip the calendar re-sync when the feeds were fetched this recently, so a
/// caller running several queries in a row does not re-download them each time.
const CALENDAR_FRESH_MS: i64 = 15 * 60 * 1000;

const USAGE: &str = "\
recall-cli: print your Recall activity timeline

USAGE:
    recall-cli timeline [OPTIONS]
    recall-cli help

OPTIONS:
    --date <DAY>        One day (sets both --from and --to)
    --from <DAY>        First day of the range
    --to <DAY>          Last day of the range (default: today)
                        With none of these, the range is yesterday.
    --format <FORMAT>   json (default) or markdown
    --no-calendar-sync  Use the calendar events already in the database
                        instead of re-fetching the iCal feeds first
    --db <PATH>         Database to read (default: the desktop app's)

DAY is YYYY-MM-DD, `today`, `yesterday` or `last-workday` (the most recent
Monday-Friday before today, so on a Monday it is the previous Friday).

Data sources and credentials are configured in the Recall app; this command
only reads them. Each event carries `logged`: whether it has been ticked off
as logged in Harvest in the app.

EXIT STATUS:
    0  the full timeline was printed
    1  bad arguments, or the database could not be read
    2  the timeline was printed, but some sources failed (listed under
       `errors`), so it is incomplete
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Json,
    Markdown,
}

#[derive(Debug, PartialEq, Eq)]
struct TimelineArgs {
    start: NaiveDate,
    end: NaiveDate,
    format: Format,
    calendar_sync: bool,
    db: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    Timeline(TimelineArgs),
}

/// Entry point for the `recall-cli` binary.
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let today = Local::now().date_naive();
    match parse_args(&args, today) {
        Ok(Command::Help) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Command::Timeline(args)) => match timeline(&args) {
            Ok((output, complete)) => {
                print!("{output}");
                if complete {
                    ExitCode::SUCCESS
                } else {
                    eprintln!("recall-cli: some sources failed; the timeline is incomplete");
                    ExitCode::from(2)
                }
            }
            Err(e) => {
                eprintln!("recall-cli: {e}");
                ExitCode::from(1)
            }
        },
        Err(e) => {
            eprintln!("recall-cli: {e}\n\nRun `recall-cli help` for usage.");
            ExitCode::from(1)
        }
    }
}

fn parse_args(args: &[String], today: NaiveDate) -> Result<Command, String> {
    let mut it = args.iter();
    match it.next().map(String::as_str) {
        None | Some("help" | "--help" | "-h") => return Ok(Command::Help),
        Some("timeline") => {}
        Some(other) => return Err(format!("unknown command `{other}`")),
    }

    let mut from = None;
    let mut to = None;
    let mut format = Format::Json;
    let mut calendar_sync = true;
    let mut db = None;
    while let Some(flag) = it.next() {
        let mut value = || {
            it.next()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match flag.as_str() {
            "--date" => {
                let day = parse_day(value()?, today)?;
                from = Some(day);
                to = Some(day);
            }
            "--from" => from = Some(parse_day(value()?, today)?),
            "--to" => to = Some(parse_day(value()?, today)?),
            "--format" => {
                format = match value()?.as_str() {
                    "json" => Format::Json,
                    "markdown" | "md" => Format::Markdown,
                    other => return Err(format!("unknown format `{other}` (use json or markdown)")),
                }
            }
            "--no-calendar-sync" => calendar_sync = false,
            "--db" => db = Some(PathBuf::from(value()?)),
            "help" | "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unknown option `{other}`")),
        }
    }

    let (start, end) = match (from, to) {
        (Some(f), Some(t)) => (f, t),
        (Some(f), None) => (f, today),
        // Only --to: just that day, rather than an unbounded range.
        (None, Some(t)) => (t, t),
        (None, None) => {
            let y = today - Duration::days(1);
            (y, y)
        }
    };
    if end < start {
        return Err(format!("--to {end} is before --from {start}"));
    }

    Ok(Command::Timeline(TimelineArgs { start, end, format, calendar_sync, db }))
}

fn parse_day(raw: &str, today: NaiveDate) -> Result<NaiveDate, String> {
    match raw {
        "today" => Ok(today),
        "yesterday" => Ok(today - Duration::days(1)),
        "last-workday" => Ok(last_workday(today)),
        _ => NaiveDate::parse_from_str(raw, "%Y-%m-%d").map_err(|_| {
            format!("invalid day `{raw}` (use YYYY-MM-DD, today, yesterday or last-workday)")
        }),
    }
}

/// The most recent Monday–Friday strictly before `today`.
fn last_workday(today: NaiveDate) -> NaiveDate {
    let mut day = today - Duration::days(1);
    while matches!(day.weekday(), Weekday::Sat | Weekday::Sun) {
        day -= Duration::days(1);
    }
    day
}

/// Fetch and render the timeline. Returns the output and whether every source
/// succeeded.
fn timeline(args: &TimelineArgs) -> Result<(String, bool), String> {
    let db_path = match &args.db {
        Some(p) => p.clone(),
        None => db::default_db_path()?,
    };
    let conn = db::open_existing(&db_path)?;
    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
        db_path,
        ical_syncing: Arc::new(AtomicBool::new(false)),
    };

    // The calendar source reads feeds the app syncs in the background while it
    // is open; when it has not run lately the stored events are stale.
    if args.calendar_sync {
        let fresh = settings_ical::last_synced_at(&state)
            .is_some_and(|at| now() - at < CALENDAR_FRESH_MS);
        if !fresh {
            settings_ical::sync_blocking(&state);
        }
    }

    let start = args.start.format("%Y-%m-%d").to_string();
    let end = args.end.format("%Y-%m-%d").to_string();
    let result = export_range(&state, &start, &end)?;
    telemetry::record(&state, "cli.timeline");

    let ids: Vec<String> = result
        .days
        .iter()
        .flat_map(|d| d.events.iter().map(|ev| ev.id.clone()))
        .collect();
    let logged: HashSet<String> = done_event_ids(&state, ids)?.into_iter().collect();

    let output = match args.format {
        Format::Json => to_json(&result, &start, &end, &logged),
        Format::Markdown => to_markdown(&result, &start, &end, &logged),
    };
    Ok((output, result.errors.is_empty()))
}

#[derive(Serialize)]
struct JsonOutput<'a> {
    range: JsonRange<'a>,
    days: Vec<JsonDay<'a>>,
    errors: &'a [ExportSourceError],
}

#[derive(Serialize)]
struct JsonRange<'a> {
    start: &'a str,
    end: &'a str,
}

#[derive(Serialize)]
struct JsonDay<'a> {
    date: &'a str,
    events: Vec<JsonEvent<'a>>,
}

/// Mirrors the app's JSON export (`src/lib/export.ts` `toJson`) plus `logged`.
#[derive(Serialize)]
struct JsonEvent<'a> {
    time: &'a str,
    source: TimelineEventSource,
    title: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
    logged: bool,
}

fn to_json(result: &ExportResult, start: &str, end: &str, logged: &HashSet<String>) -> String {
    let out = JsonOutput {
        range: JsonRange { start, end },
        days: result
            .days
            .iter()
            .map(|day| JsonDay {
                date: &day.date,
                events: day
                    .events
                    .iter()
                    .map(|ev| JsonEvent {
                        time: &ev.time,
                        source: ev.source,
                        title: &ev.title,
                        detail: ev.detail.as_deref().filter(|d| !d.is_empty()),
                        url: ev.url.as_deref().filter(|u| !u.is_empty()),
                        logged: logged.contains(&ev.id),
                    })
                    .collect(),
            })
            .collect(),
        errors: &result.errors,
    };
    let mut json = serde_json::to_string_pretty(&out).expect("timeline serializes");
    json.push('\n');
    json
}

fn source_label(source: TimelineEventSource) -> &'static str {
    match source {
        TimelineEventSource::Git => "Git",
        TimelineEventSource::Github => "GitHub",
        TimelineEventSource::Calendar => "Calendar",
        TimelineEventSource::Gmail => "Gmail",
        TimelineEventSource::Drive => "Drive",
        TimelineEventSource::Jira => "Jira",
        TimelineEventSource::Zulip => "Zulip",
    }
}

fn event_markdown(ev: &TimelineEvent, logged: bool) -> String {
    let mut line = format!("- **{}** `{}` — {}", ev.time, source_label(ev.source), ev.title);
    if let Some(detail) = &ev.detail {
        // Collapse newlines so a multi-line detail stays on one bullet.
        let detail = detail.split_whitespace().collect::<Vec<_>>().join(" ");
        if !detail.is_empty() {
            line.push_str(&format!(" — {detail}"));
        }
    }
    if let Some(url) = ev.url.as_deref().filter(|u| !u.is_empty()) {
        line.push_str(&format!(" ({url})"));
    }
    if logged {
        line.push_str(" — _logged in Harvest_");
    }
    line
}

/// Mirrors the app's Markdown export (`src/lib/export.ts` `toMarkdown`, English
/// labels, no prompt), plus a note on failed sources and logged events.
fn to_markdown(result: &ExportResult, start: &str, end: &str, logged: &HashSet<String>) -> String {
    let range = if start == end { start.to_string() } else { format!("{start} → {end}") };
    let mut lines = vec![
        format!("# Activity timeline ({range})"),
        String::new(),
        "_Exported from Recall, a personal work-tracking app. Times are local. Each bullet is one tracked activity (commit, PR, calendar event, ticket update, message, …)._".to_string(),
        String::new(),
    ];

    if !result.errors.is_empty() {
        lines.push("> **Incomplete:** these sources failed, so their activity is missing:".to_string());
        for e in &result.errors {
            lines.push(format!("> - {}: {}", e.source, e.error));
        }
        lines.push(String::new());
    }

    for day in &result.days {
        let heading = NaiveDate::parse_from_str(&day.date, "%Y-%m-%d")
            .map(|d| d.format("%A, %B %-d").to_string())
            .unwrap_or_default();
        lines.push(format!("## {} — {heading}", day.date));
        lines.push(String::new());
        if day.events.is_empty() {
            lines.push("_No tracked activity._".to_string());
            lines.push(String::new());
            continue;
        }
        for ev in &day.events {
            lines.push(event_markdown(ev, logged.contains(&ev.id)));
        }
        lines.push(String::new());
    }

    let mut out = lines.join("\n").trim_end().to_string();
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::timeline::ExportDay;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    // 2026-10-29 is a Thursday.
    const THURSDAY: &str = "2026-10-29";

    fn parse_timeline(list: &[&str]) -> TimelineArgs {
        match parse_args(&args(list), d(THURSDAY)).unwrap() {
            Command::Timeline(t) => t,
            Command::Help => panic!("expected a timeline command"),
        }
    }

    fn ev(id: &str, time: &str, source: TimelineEventSource, title: &str) -> TimelineEvent {
        TimelineEvent {
            id: id.into(),
            time: time.into(),
            timestamp: 0,
            source,
            title: title.into(),
            detail: None,
            url: None,
        }
    }

    #[test]
    fn no_command_or_help_prints_usage() {
        for list in [&[][..], &["help"], &["--help"], &["timeline", "-h"]] {
            assert_eq!(parse_args(&args(list), d(THURSDAY)).unwrap(), Command::Help);
        }
    }

    #[test]
    fn unknown_commands_options_and_formats_are_errors() {
        let today = d(THURSDAY);
        assert!(parse_args(&args(&["timelines"]), today).is_err());
        assert!(parse_args(&args(&["timeline", "--verbose"]), today).is_err());
        assert!(parse_args(&args(&["timeline", "--format", "csv"]), today).is_err());
        assert!(parse_args(&args(&["timeline", "--date"]), today).is_err());
        assert!(parse_args(&args(&["timeline", "--date", "28-10-2026"]), today).is_err());
    }

    #[test]
    fn timeline_defaults_to_yesterday_as_json_with_calendar_sync() {
        let t = parse_timeline(&["timeline"]);
        assert_eq!(
            t,
            TimelineArgs {
                start: d("2026-10-28"),
                end: d("2026-10-28"),
                format: Format::Json,
                calendar_sync: true,
                db: None,
            }
        );
    }

    #[test]
    fn date_sets_a_single_day_and_options_parse() {
        let t = parse_timeline(&[
            "timeline", "--date", "2026-10-20", "--format", "markdown", "--no-calendar-sync", "--db", "/tmp/x.sqlite",
        ]);
        assert_eq!((t.start, t.end), (d("2026-10-20"), d("2026-10-20")));
        assert_eq!(t.format, Format::Markdown);
        assert!(!t.calendar_sync);
        assert_eq!(t.db, Some(PathBuf::from("/tmp/x.sqlite")));
    }

    #[test]
    fn from_alone_runs_to_today_and_to_alone_is_one_day() {
        let t = parse_timeline(&["timeline", "--from", "2026-10-26"]);
        assert_eq!((t.start, t.end), (d("2026-10-26"), d(THURSDAY)));
        let t = parse_timeline(&["timeline", "--to", "2026-10-26"]);
        assert_eq!((t.start, t.end), (d("2026-10-26"), d("2026-10-26")));
    }

    #[test]
    fn inverted_range_is_an_error() {
        let err = parse_args(&args(&["timeline", "--from", "today", "--to", "yesterday"]), d(THURSDAY))
            .unwrap_err();
        assert!(err.contains("before"), "{err}");
    }

    #[test]
    fn relative_days_resolve_against_today() {
        let today = d(THURSDAY);
        assert_eq!(parse_day("today", today).unwrap(), today);
        assert_eq!(parse_day("yesterday", today).unwrap(), d("2026-10-28"));
        assert_eq!(parse_day("last-workday", today).unwrap(), d("2026-10-28"));
    }

    #[test]
    fn last_workday_skips_the_weekend() {
        // Monday, Sunday and Saturday all look back to Friday 2026-10-23.
        for today in ["2026-10-26", "2026-10-25", "2026-10-24"] {
            assert_eq!(last_workday(d(today)), d("2026-10-23"), "from {today}");
        }
        // Tuesday looks back to Monday.
        assert_eq!(last_workday(d("2026-10-27")), d("2026-10-26"));
    }

    fn sample() -> ExportResult {
        let mut commit = ev("git:abc", "09:05", TimelineEventSource::Git, "Fix login");
        commit.detail = Some("recall\n  main".into());
        commit.url = Some("https://example.com/c/abc".into());
        ExportResult {
            days: vec![
                ExportDay {
                    date: "2026-10-27".into(),
                    events: vec![
                        commit,
                        ev("jira:LB-1", "11:30", TimelineEventSource::Jira, "LB-1 Commented"),
                    ],
                },
                ExportDay { date: "2026-10-28".into(), events: vec![] },
            ],
            errors: vec![],
        }
    }

    #[test]
    fn markdown_matches_the_app_export_and_marks_logged_events() {
        let logged: HashSet<String> = ["jira:LB-1".to_string()].into();
        let md = to_markdown(&sample(), "2026-10-27", "2026-10-28", &logged);
        assert!(md.starts_with("# Activity timeline (2026-10-27 → 2026-10-28)\n"));
        assert!(md.contains("## 2026-10-27 — Tuesday, October 27\n"));
        assert!(md.contains("- **09:05** `Git` — Fix login — recall main (https://example.com/c/abc)\n"));
        assert!(md.contains("- **11:30** `Jira` — LB-1 Commented — _logged in Harvest_\n"));
        assert!(md.contains("## 2026-10-28 — Wednesday, October 28\n\n_No tracked activity._\n"));
        assert!(!md.contains("Incomplete"));
        assert!(md.ends_with("_No tracked activity._\n"));
    }

    #[test]
    fn markdown_lists_failed_sources_up_front() {
        let mut result = sample();
        result.errors.push(ExportSourceError { source: "Jira".into(), error: "401".into() });
        let md = to_markdown(&result, "2026-10-27", "2026-10-28", &HashSet::new());
        assert!(md.contains("> **Incomplete:**"));
        assert!(md.contains("> - Jira: 401\n"));
    }

    #[test]
    fn json_has_range_days_logged_flag_and_errors() {
        let mut result = sample();
        result.errors.push(ExportSourceError { source: "Zulip".into(), error: "timeout".into() });
        let logged: HashSet<String> = ["git:abc".to_string()].into();
        let v: serde_json::Value =
            serde_json::from_str(&to_json(&result, "2026-10-27", "2026-10-28", &logged)).unwrap();
        assert_eq!(v["range"], serde_json::json!({ "start": "2026-10-27", "end": "2026-10-28" }));
        let first = &v["days"][0]["events"][0];
        assert_eq!(first["source"], "git");
        assert_eq!(first["logged"], true);
        assert_eq!(first["url"], "https://example.com/c/abc");
        let second = &v["days"][0]["events"][1];
        assert_eq!(second["logged"], false);
        assert!(second.get("detail").is_none() && second.get("url").is_none());
        assert_eq!(v["days"][1]["events"], serde_json::json!([]));
        assert_eq!(v["errors"][0]["source"], "Zulip");
    }

    #[test]
    fn timeline_reads_an_existing_database_without_network() {
        let dir = std::env::temp_dir().join(format!("recall-cli-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("db.sqlite");
        db::init_schema(&rusqlite::Connection::open(&path).unwrap()).unwrap();

        // No sources configured: every source returns empty without a fetch.
        let t = TimelineArgs {
            start: d("2026-10-27"),
            end: d("2026-10-28"),
            format: Format::Json,
            calendar_sync: true,
            db: Some(path.clone()),
        };
        let (out, complete) = timeline(&t).unwrap();
        assert!(complete);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["days"].as_array().unwrap().len(), 2);

        let conn = rusqlite::Connection::open(&path).unwrap();
        let counted: i64 = conn
            .query_row("SELECT value FROM telemetry_counters WHERE key = 'cli.timeline'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(counted, 1);
        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
