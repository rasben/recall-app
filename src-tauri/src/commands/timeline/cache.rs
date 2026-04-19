//! SQLite-backed cache for a full day's merged timeline events.
//!
//! We only cache days that have already ended (strictly before the local "today"),
//! so the current day and future days always go through the live data-source fetch.
//! This keeps "reopen the app and see your last week" fast, while today's view
//! stays fresh as activity lands.

use rusqlite::params;
use tauri::State;

use crate::commands::settings::now;
use crate::state::AppState;
use crate::timeline::TimelineEvent;

pub(super) fn get_cached_day(
    state: &State<'_, AppState>,
    day: &str,
) -> Option<Vec<TimelineEvent>> {
    let conn = state.db.lock().ok()?;
    let mut stmt = conn
        .prepare("SELECT events_json FROM timeline_day_cache WHERE day = ?1")
        .ok()?;
    let json: String = stmt.query_row(params![day], |row| row.get(0)).ok()?;
    serde_json::from_str(&json).ok()
}

pub(super) fn save_cached_day(
    state: &State<'_, AppState>,
    day: &str,
    events: &[TimelineEvent],
) -> Result<(), String> {
    let json = serde_json::to_string(events).map_err(|e| format!("serialize cache: {e}"))?;
    let conn = state.db.lock().map_err(|_| "Failed to access database")?;
    conn.execute(
        "INSERT OR REPLACE INTO timeline_day_cache (day, events_json, updated_at) VALUES (?1, ?2, ?3)",
        params![day, json, now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
