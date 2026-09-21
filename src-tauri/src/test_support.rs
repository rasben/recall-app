//! Shared helpers for unit tests that need a real `State<'_, AppState>`.
//!
//! `tauri::test::mock_app()` builds an app on the `MockRuntime` (no window, no
//! webview), so tests can manage an `AppState` backed by an in-memory SQLite
//! database carrying the production schema and then call command functions
//! exactly as the frontend would.

use std::sync::{atomic::AtomicBool, Arc, Mutex};

use rusqlite::Connection;
use tauri::test::MockRuntime;
use tauri::{App, Manager, State};

use crate::state::AppState;
use crate::timeline::{TimelineEvent, TimelineEventSource};

/// A mock app managing an `AppState` whose DB is a fresh in-memory SQLite
/// connection with every table created. `db_path` points at a file that does
/// not exist, so code that inspects the on-disk size sees 0 bytes.
pub(crate) fn mock_app() -> App<MockRuntime> {
    let db_path = std::env::temp_dir().join(format!(
        "recall-test-{}-nonexistent.sqlite",
        uuid::Uuid::new_v4()
    ));
    mock_app_at(db_path)
}

/// Like [`mock_app`], but with `AppState::db_path` set to `db_path`. The DB
/// itself is still in-memory; the path is only what size/telemetry-style code
/// looks at.
pub(crate) fn mock_app_at(db_path: std::path::PathBuf) -> App<MockRuntime> {
    let conn = Connection::open_in_memory().expect("open in-memory sqlite");
    crate::db::init_schema(&conn).expect("init schema");
    let app = tauri::test::mock_app();
    app.manage(AppState {
        db: Arc::new(Mutex::new(conn)),
        db_path,
        ical_syncing: Arc::new(AtomicBool::new(false)),
    });
    app
}

pub(crate) fn state(app: &App<MockRuntime>) -> State<'_, AppState> {
    app.state::<AppState>()
}

/// Store a raw JSON settings document under `key`, bypassing the typed
/// `set_settings_*` commands (so tests can also seed legacy or partial shapes).
pub(crate) fn seed_setting(state: &State<'_, AppState>, key: &str, json: &str) {
    crate::commands::settings::save_val(state, key, json).expect("seed setting");
}

/// Minimal event with the given id/timestamp; `time` is derived as `HH:MM`
/// from the timestamp in the local zone, like every real source does.
pub(crate) fn event(id: &str, timestamp: i64, source: TimelineEventSource) -> TimelineEvent {
    use chrono::{DateTime, Local};
    let time = DateTime::from_timestamp(timestamp, 0)
        .map(|d| d.with_timezone(&Local).format("%H:%M").to_string())
        .unwrap_or_else(|| "00:00".to_string());
    TimelineEvent {
        id: id.to_string(),
        time,
        timestamp,
        source,
        title: format!("title {id}"),
        detail: None,
        url: None,
    }
}

/// Unix seconds for `hh:mm` local time on `day`.
pub(crate) fn local_ts(day: chrono::NaiveDate, hh: u32, mm: u32) -> i64 {
    use chrono::{Local, TimeZone};
    Local
        .from_local_datetime(&day.and_hms_opt(hh, mm, 0).unwrap())
        .single()
        .expect("unambiguous local time")
        .timestamp()
}

/// Multi-thread Tokio runtime: the async commands call
/// `tokio::task::block_in_place`, which panics on a current-thread runtime.
pub(crate) fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime")
}
