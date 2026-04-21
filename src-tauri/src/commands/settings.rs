use crate::state::AppState;
use rusqlite::params;
use serde::Serialize;
use specta::Type;
use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

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
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub(crate) fn get_val(state: &State<'_, AppState>, key: &str) -> Option<String> {
    let conn = state.db.lock().ok()?;

    let mut stmt = conn
        .prepare("SELECT value FROM settings WHERE key = ?1")
        .ok()?;

    stmt.query_row(params![key], |row| row.get(0)).ok()
}

#[derive(Serialize, Type)]
pub struct ClearCachesResult {
    pub rows_deleted: u32,
}

#[derive(Serialize, Type)]
pub struct CacheSizeResult {
    pub bytes: u32,
    pub cached_days: u32,
}

#[tauri::command]
#[specta::specta]
pub fn get_cached_day_event_counts(state: State<'_, AppState>) -> Result<HashMap<String, u32>, String> {
    let conn = state.db.lock().map_err(|_| "DB lock failed".to_string())?;
    let mut stmt = conn
        .prepare("SELECT day, json_array_length(events_json) FROM timeline_day_cache")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?)))
        .map_err(|e| e.to_string())?;
    let mut counts = HashMap::new();
    for row in rows {
        let (day, count) = row.map_err(|e| e.to_string())?;
        counts.insert(day, count);
    }
    Ok(counts)
}

#[tauri::command]
#[specta::specta]
pub fn get_cache_size(state: State<'_, AppState>) -> Result<CacheSizeResult, String> {
    let cached_days: u32 = {
        let conn = state.db.lock().map_err(|_| "DB lock failed".to_string())?;
        conn.query_row("SELECT COUNT(*) FROM timeline_day_cache", [], |r| r.get(0))
            .map_err(|e| e.to_string())?
    };
    let bytes = fs::metadata(&state.db_path).map(|m| m.len() as u32).unwrap_or(0);
    Ok(CacheSizeResult { bytes, cached_days })
}

#[tauri::command]
#[specta::specta]
pub fn clear_all_caches(state: State<'_, AppState>) -> Result<ClearCachesResult, String> {
    let conn = state.db.lock().map_err(|_| "DB lock failed".to_string())?;
    let mut rows_deleted = 0u32;
    rows_deleted += conn
        .execute("DELETE FROM timeline_day_cache", [])
        .map_err(|e| e.to_string())? as u32;
    rows_deleted += conn
        .execute("DELETE FROM ical_events", [])
        .map_err(|e| e.to_string())? as u32;
    Ok(ClearCachesResult { rows_deleted })
}
