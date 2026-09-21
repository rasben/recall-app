//! Privacy-first usage telemetry.
//!
//! The app never streams events. Instead it accumulates **counters** locally
//! (`telemetry_counters` table) and, at most once per calendar day, POSTs one
//! JSON summary to a Cloudflare Worker: environment (version, OS family, arch,
//! language, theme), a configuration snapshot (which sources are enabled, as
//! booleans and bucketed counts), and the accumulated counters. Counters are
//! reset after a successful send.
//!
//! What is never sent: titles, URLs, repo names or paths, ticket keys,
//! usernames, emails, calendar summaries, hostnames, error message text,
//! timezone, or any timestamp finer than the calendar day. The only identifier
//! is a random UUID generated at first launch.
//!
//! All errors are silently swallowed; telemetry must never affect the user.
//!
//! To deploy the worker and fill in the URL, see `worker/wrangler.toml`.
use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::State;

use crate::commands::settings::save_val;
use crate::state::AppState;

const PING_URL: &str = "https://recall-telemetry.recall-app.workers.dev/ping";

/// Bumped whenever the payload shape changes so the worker / queries can tell
/// pings apart. Schema 1 was the bare `{ id }` install ping.
pub const SCHEMA_VERSION: u32 = 2;

const KEY_INSTALL_ID: &str = "telemetry_install_id";
const KEY_LAST_PING: &str = "telemetry_last_ping_date";
/// Gauges (current state, not counts) live in the `settings` table under this
/// prefix, e.g. `telemetry_gauge_lang = "da"`.
const GAUGE_PREFIX: &str = "telemetry_gauge_";

/// How often the background thread re-checks whether a new calendar day has
/// started, so a long-running app still pings daily.
const RECHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);
const MAX_KEY_LEN: usize = 64;
const MAX_GAUGE_LEN: usize = 32;

// ─── Recording ──────────────────────────────────────────────────────────────

/// Counter keys are dotted lowercase identifiers (`nav.prev`, `error.github.auth`).
/// Anything else is dropped so a typo or a stray value can never leak into the
/// payload.
pub fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= MAX_KEY_LEN
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
}

/// Increment `key` by one on an already-held connection. Use this when the
/// caller holds the app DB lock (calling [`record`] there would deadlock).
pub fn bump_conn(conn: &Connection, key: &str) {
    bump_conn_by(conn, key, 1);
}

fn bump_conn_by(conn: &Connection, key: &str, n: i64) {
    if !valid_key(key) {
        #[cfg(debug_assertions)]
        eprintln!("[telemetry] dropped invalid counter key {key:?}");
        return;
    }
    let _ = conn.execute(
        "INSERT INTO telemetry_counters (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = value + excluded.value",
        params![key, n],
    );
}

/// Increment `key` using the shared app connection. Never fails.
pub fn record(state: &AppState, key: &str) {
    if let Ok(conn) = state.db.lock() {
        bump_conn(&conn, key);
    }
}

/// Store a gauge (current state such as `lang = "da"`). Both name and value
/// must be short lowercase identifiers; anything else is dropped.
pub fn set_gauge(state: &State<'_, AppState>, name: &str, value: &str) {
    if !valid_key(name) || !valid_key(value) || value.len() > MAX_GAUGE_LEN {
        #[cfg(debug_assertions)]
        eprintln!("[telemetry] dropped invalid gauge {name:?}={value:?}");
        return;
    }
    let _ = save_val(state, &format!("{GAUGE_PREFIX}{name}"), value);
}

/// Record how long a source took for one fetch. `prefix` is `load_ms` for a
/// single-day load and `range_ms` for a multi-day range fetch.
pub fn record_source_timing(state: &AppState, prefix: &str, source: &str, elapsed: Duration) {
    let key = format!(
        "{prefix}.{}.{}",
        source.to_ascii_lowercase(),
        duration_bucket(elapsed.as_millis() as u64)
    );
    record(state, &key);
}

/// Record how many events a source returned for one day.
pub fn record_source_events(state: &AppState, source: &str, count: usize) {
    let key = format!(
        "events.{}.{}",
        source.to_ascii_lowercase(),
        events_bucket(count)
    );
    record(state, &key);
}

/// Record a source failure by coarse class only; the message itself is never stored.
pub fn record_source_error(state: &AppState, source: &str, error: &str) {
    let key = format!(
        "error.{}.{}",
        source.to_ascii_lowercase(),
        classify_error(error)
    );
    record(state, &key);
}

/// Map an error message onto a handful of classes. Only the class is sent.
pub fn classify_error(message: &str) -> &'static str {
    let m = message.to_ascii_lowercase();
    if m.contains("401")
        || m.contains("403")
        || m.contains("unauthori")
        || m.contains("forbidden")
        || m.contains("authentication")
        || m.contains("invalid token")
        || m.contains("bad credentials")
    {
        "auth"
    } else if m.contains("429") || m.contains("rate limit") || m.contains("too many requests") {
        "rate_limit"
    } else if m.contains("timed out")
        || m.contains("timeout")
        || m.contains("dns")
        || m.contains("connection")
        || m.contains("connect")
        || m.contains("network")
        || m.contains("io error")
    {
        "network"
    } else if m.contains("required")
        || m.contains("not a directory")
        || m.contains("not configured")
        || m.contains("missing")
        || m.contains("not found")
    {
        "config"
    } else {
        "other"
    }
}

pub fn duration_bucket(ms: u64) -> &'static str {
    match ms {
        0..=999 => "lt1s",
        1000..=2999 => "1to3s",
        3000..=9999 => "3to10s",
        _ => "gt10s",
    }
}

pub fn events_bucket(n: usize) -> &'static str {
    match n {
        0 => "0",
        1..=10 => "1to10",
        11..=50 => "11to50",
        _ => "50plus",
    }
}

fn count_bucket(n: usize) -> &'static str {
    match n {
        0 => "0",
        1 => "1",
        2 => "2",
        _ => "3plus",
    }
}

fn install_age_bucket(first_seen_ms: i64, now_ms: i64) -> &'static str {
    let days = (now_ms - first_seen_ms).max(0) / 86_400_000;
    match days {
        0..=7 => "0to7d",
        8..=30 => "8to30d",
        31..=90 => "31to90d",
        91..=365 => "91to365d",
        _ => "365dplus",
    }
}

// ─── Daily ping ─────────────────────────────────────────────────────────────

/// Start the background telemetry thread. Counts this launch, then pings once
/// per calendar day for as long as the app runs. Call once from `app.setup()`.
pub fn spawn_ping(db_path: PathBuf) {
    let _ = std::thread::Builder::new()
        .name("telemetry".into())
        .spawn(move || {
            if let Ok(conn) = open(&db_path) {
                bump_conn(&conn, "app.launch");
            }
            loop {
                if let Err(e) = ping(&db_path) {
                    #[cfg(debug_assertions)]
                    eprintln!("[telemetry] ping failed: {e}");
                    let _ = e;
                }
                std::thread::sleep(RECHECK_INTERVAL);
            }
        });
}

fn open(db_path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(db_path)?;
    // The main app connection may be mid-write; wait instead of failing.
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(conn)
}

fn ping(db_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Skip if the URL hasn't been filled in yet (dev / before worker deploy).
    if PING_URL.contains("REPLACE_ME") {
        return Ok(());
    }

    let conn = open(db_path)?;

    let today = Local::now().format("%Y-%m-%d").to_string();
    if get_setting(&conn, KEY_LAST_PING).as_deref() == Some(today.as_str()) {
        return Ok(());
    }

    let (install_id, first_seen_ms) = install_identity(&conn)?;
    let counters = read_counters(&conn)?;
    let payload = build_payload(&conn, &install_id, &today, first_seen_ms, now_ms(), &counters);

    // ureq 3.x returns Err for non-2xx, so `?` handles the error path; if we
    // reach the next line the request succeeded.
    ureq::post(PING_URL)
        .header("Content-Type", "application/json")
        .send(payload.to_string())?;

    set_setting(&conn, KEY_LAST_PING, &today)?;
    // Subtract what was sent rather than deleting, so increments that landed
    // while the request was in flight are kept for tomorrow.
    subtract_counters(&conn, &counters)?;

    Ok(())
}

/// The random install id plus the time it was first created (ms since epoch),
/// creating both on first call.
fn install_identity(conn: &Connection) -> rusqlite::Result<(String, i64)> {
    let existing = conn
        .query_row(
            "SELECT value, updated_at FROM settings WHERE key = ?1",
            params![KEY_INSTALL_ID],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    if let Some(found) = existing {
        return Ok(found);
    }
    let id = uuid::Uuid::new_v4().to_string();
    let ts = now_ms();
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
        params![KEY_INSTALL_ID, id, ts],
    )?;
    Ok((id, ts))
}

fn read_counters(conn: &Connection) -> rusqlite::Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare("SELECT key, value FROM telemetry_counters WHERE value > 0 ORDER BY key")?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    rows.collect()
}

fn subtract_counters(conn: &Connection, sent: &[(String, i64)]) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (key, value) in sent {
        tx.execute(
            "UPDATE telemetry_counters SET value = value - ?2 WHERE key = ?1",
            params![key, value],
        )?;
    }
    tx.execute("DELETE FROM telemetry_counters WHERE value <= 0", [])?;
    tx.commit()
}

/// Assemble the daily summary. Pure over its inputs apart from reading the
/// settings snapshot from `conn`, so tests can assert on exactly what leaves
/// the machine.
fn build_payload(
    conn: &Connection,
    install_id: &str,
    date: &str,
    first_seen_ms: i64,
    now_ms: i64,
    counters: &[(String, i64)],
) -> Value {
    let ui = settings_json(conn, "settings_ui");
    let mut counter_map = Map::new();
    for (key, value) in counters {
        counter_map.insert(key.clone(), json!(value));
    }
    json!({
        "id": install_id,
        "date": date,
        "schema": SCHEMA_VERSION,
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "lang": gauge(conn, "lang"),
        "theme": sanitized_str(ui.get("theme")),
        "group_mode": gauge(conn, "group_mode"),
        "install_age": install_age_bucket(first_seen_ms, now_ms),
        "config": config_snapshot(conn),
        "counters": Value::Object(counter_map),
    })
}

/// Which features are configured, as booleans and buckets only. No values.
fn config_snapshot(conn: &Connection) -> Value {
    let git = settings_json(conn, "settings_git");
    let github = settings_json(conn, "settings_github");
    let ical = settings_json(conn, "settings_ical");
    let jira = settings_json(conn, "settings_jira");
    let zulip = settings_json(conn, "settings_zulip");
    let export = settings_json(conn, "settings_export");

    let ical_urls = ical
        .get("urls")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|u| u.as_str().is_some_and(|s| !s.trim().is_empty()))
                .count()
        })
        .unwrap_or(0);
    let custom_prompt = export
        .get("prompt")
        .and_then(Value::as_str)
        .is_some_and(|p| !p.trim().is_empty());

    json!({
        "sources": {
            "git": flag(&git, "enabled"),
            "github": flag(&github, "enabled"),
            "calendar": flag(&ical, "enabled"),
            "jira": flag(&jira, "enabled"),
            "zulip": flag(&zulip, "enabled"),
        },
        "github_events": enum_names(github.get("enabled_events")),
        "jira_events": enum_names(jira.get("enabled_events")),
        "ical_urls": count_bucket(ical_urls),
        "custom_export_prompt": custom_prompt,
    })
}

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Serde-serialized unit enum variants come through as plain strings; keep
/// only those, and only if they look like identifiers.
fn enum_names(v: Option<&Value>) -> Value {
    let names: Vec<&str> = v
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .filter(|s| s.len() <= MAX_GAUGE_LEN && s.chars().all(|c| c.is_ascii_alphanumeric()))
                .collect()
        })
        .unwrap_or_default();
    json!(names)
}

/// Pass a short identifier-like string through; anything else becomes null so
/// a free-text field can never ride along.
fn sanitized_str(v: Option<&Value>) -> Value {
    match v.and_then(Value::as_str) {
        Some(s) if valid_key(s) && s.len() <= MAX_GAUGE_LEN => json!(s),
        _ => Value::Null,
    }
}

fn gauge(conn: &Connection, name: &str) -> Value {
    match get_setting(conn, &format!("{GAUGE_PREFIX}{name}")) {
        Some(s) if valid_key(&s) && s.len() <= MAX_GAUGE_LEN => json!(s),
        _ => Value::Null,
    }
}

fn settings_json(conn: &Connection, key: &str) -> Value {
    get_setting(conn, key)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}

fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .ok()
}

fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
        params![key, value, now_ms()],
    )?;
    Ok(())
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at INTEGER NOT NULL);
             CREATE TABLE telemetry_counters (key TEXT PRIMARY KEY, value INTEGER NOT NULL DEFAULT 0);",
        )
        .unwrap();
        conn
    }

    fn counter(conn: &Connection, key: &str) -> i64 {
        conn.query_row(
            "SELECT value FROM telemetry_counters WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .unwrap_or(0)
    }

    #[test]
    fn valid_key_accepts_dotted_lowercase_only() {
        assert!(valid_key("nav.prev"));
        assert!(valid_key("error.github.rate_limit"));
        assert!(valid_key("load_ms.git.lt1s"));
        assert!(!valid_key(""));
        assert!(!valid_key("Nav.Prev"));
        assert!(!valid_key("nav prev"));
        assert!(!valid_key("nav/prev"));
        assert!(!valid_key(&"a".repeat(MAX_KEY_LEN + 1)));
    }

    #[test]
    fn bump_accumulates_and_ignores_invalid_keys() {
        let conn = mem_db();
        bump_conn(&conn, "nav.prev");
        bump_conn(&conn, "nav.prev");
        bump_conn(&conn, "nav.next");
        bump_conn(&conn, "BAD KEY");
        assert_eq!(counter(&conn, "nav.prev"), 2);
        assert_eq!(counter(&conn, "nav.next"), 1);
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM telemetry_counters", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 2);
    }

    #[test]
    fn subtract_keeps_increments_made_after_snapshot() {
        let conn = mem_db();
        bump_conn(&conn, "nav.prev");
        bump_conn(&conn, "nav.prev");
        let snapshot = read_counters(&conn).unwrap();
        assert_eq!(snapshot, vec![("nav.prev".to_string(), 2)]);
        // A click lands while the request is in flight.
        bump_conn(&conn, "nav.prev");
        bump_conn(&conn, "nav.today");
        subtract_counters(&conn, &snapshot).unwrap();
        assert_eq!(counter(&conn, "nav.prev"), 1);
        assert_eq!(counter(&conn, "nav.today"), 1);
    }

    #[test]
    fn classify_error_buckets_common_messages() {
        assert_eq!(classify_error("GitHub API returned 401 Unauthorized"), "auth");
        assert_eq!(classify_error("Bad credentials"), "auth");
        assert_eq!(classify_error("HTTP 429 rate limit exceeded"), "rate_limit");
        assert_eq!(classify_error("connection timed out"), "network");
        assert_eq!(classify_error("dns error: no such host"), "network");
        assert_eq!(classify_error("Scan path is required"), "config");
        assert_eq!(classify_error("Git scan path is not a directory: /x"), "config");
        assert_eq!(classify_error("something odd"), "other");
    }

    #[test]
    fn buckets_have_expected_edges() {
        assert_eq!(duration_bucket(0), "lt1s");
        assert_eq!(duration_bucket(999), "lt1s");
        assert_eq!(duration_bucket(1000), "1to3s");
        assert_eq!(duration_bucket(2999), "1to3s");
        assert_eq!(duration_bucket(3000), "3to10s");
        assert_eq!(duration_bucket(10_000), "gt10s");

        assert_eq!(events_bucket(0), "0");
        assert_eq!(events_bucket(10), "1to10");
        assert_eq!(events_bucket(11), "11to50");
        assert_eq!(events_bucket(51), "50plus");

        assert_eq!(count_bucket(0), "0");
        assert_eq!(count_bucket(3), "3plus");

        let day = 86_400_000;
        assert_eq!(install_age_bucket(0, 0), "0to7d");
        assert_eq!(install_age_bucket(0, 7 * day), "0to7d");
        assert_eq!(install_age_bucket(0, 8 * day), "8to30d");
        assert_eq!(install_age_bucket(0, 91 * day), "91to365d");
        assert_eq!(install_age_bucket(0, 400 * day), "365dplus");
        // Clock skew must not underflow.
        assert_eq!(install_age_bucket(10 * day, 0), "0to7d");
    }

    #[test]
    fn payload_contains_flags_and_counters_but_no_secrets() {
        let conn = mem_db();
        set_setting(
            &conn,
            "settings_github",
            r#"{"enabled":true,"username":"octocat-secret","token":"ghp_verysecret","enabled_events":["PullRequestEvent","PushEvent"]}"#,
        )
        .unwrap();
        set_setting(
            &conn,
            "settings_ical",
            r#"{"enabled":true,"urls":["https://calendar.example/secret.ics",""],"emails":["me@example.com"]}"#,
        )
        .unwrap();
        set_setting(&conn, "settings_git", r#"{"enabled":false,"path":"/Users/someone/code"}"#).unwrap();
        set_setting(&conn, "settings_export", r#"{"prompt":"My custom prompt text"}"#).unwrap();
        set_setting(&conn, "settings_ui", r#"{"theme":"dark"}"#).unwrap();
        set_setting(&conn, "telemetry_gauge_lang", "da").unwrap();
        // A gauge that somehow holds free text must not pass through.
        set_setting(&conn, "telemetry_gauge_group_mode", "Task View!").unwrap();

        let counters = vec![("nav.prev".to_string(), 3), ("harvest.mark".to_string(), 1)];
        let payload = build_payload(&conn, "00000000-0000-4000-8000-000000000000", "2026-09-21", 0, 40 * 86_400_000, &counters);
        let text = payload.to_string();

        assert_eq!(payload["schema"], SCHEMA_VERSION);
        assert_eq!(payload["date"], "2026-09-21");
        assert_eq!(payload["install_age"], "31to90d");
        assert_eq!(payload["lang"], "da");
        assert_eq!(payload["theme"], "dark");
        assert!(payload["group_mode"].is_null());
        assert_eq!(payload["config"]["sources"]["github"], true);
        assert_eq!(payload["config"]["sources"]["git"], false);
        assert_eq!(payload["config"]["sources"]["jira"], false);
        assert_eq!(payload["config"]["github_events"], json!(["PullRequestEvent", "PushEvent"]));
        assert_eq!(payload["config"]["ical_urls"], "1");
        assert_eq!(payload["config"]["custom_export_prompt"], true);
        assert_eq!(payload["counters"]["nav.prev"], 3);
        assert_eq!(payload["counters"]["harvest.mark"], 1);

        for secret in ["octocat", "ghp_", "calendar.example", "me@example.com", "/Users/someone", "custom prompt"] {
            assert!(!text.contains(secret), "payload leaked {secret:?}: {text}");
        }
    }

    #[test]
    fn install_identity_is_created_once_and_keeps_first_seen() {
        let conn = mem_db();
        let (id1, ts1) = install_identity(&conn).unwrap();
        let (id2, ts2) = install_identity(&conn).unwrap();
        assert_eq!(id1, id2);
        assert_eq!(ts1, ts2);
        assert!(uuid::Uuid::parse_str(&id1).is_ok());
    }
}
