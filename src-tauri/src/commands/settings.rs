use std::time::{SystemTime, UNIX_EPOCH};
use rusqlite::params;
use tauri::State;
use crate::state::AppState;

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub(crate) fn save_val(state: &State<'_, AppState>, key: &str, value: &str) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "Failed to access database")?;

    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
        params![key, value, now()],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

pub(crate) fn get_val(state: &State<'_, AppState>, key: &str) -> Option<String> {
    let conn = state.db.lock().ok()?;

    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1").ok()?;

    stmt.query_row(params![key], |row| row.get(0)).ok()
}
