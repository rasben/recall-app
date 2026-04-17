use std::time::{SystemTime, UNIX_EPOCH};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;
use crate::state::AppState;

const KEY_LANGUAGE: &str = "lang";
const KEY_THEME: &str = "theme";
const KEY_GIT_ENABLED: &str = "source_git_enabled";
const KEY_GIT_SCAN_PATH: &str = "source_git_scan_path";

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Type)]
#[specta(export = false)]
#[derive(PartialEq)]
pub enum Language {
    Danish,
    English,
}

#[tauri::command]
#[specta::specta]
pub fn get_language(state: State<'_, AppState>) -> Option<Language> {
    let s = get_val(&state, KEY_LANGUAGE)?;
    serde_json::from_str(&s).ok()
}

#[tauri::command]
#[specta::specta]
pub fn set_language(state: State<'_, AppState>, language: Language) -> Result<(), String> {
    let s = serde_json::to_string(&language).map_err(|e| e.to_string())?;
    save_val(&state, KEY_LANGUAGE, &s)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_theme(state: State<'_, AppState>) -> Option<String> {
    get_val(&state, KEY_THEME)
}

#[tauri::command]
#[specta::specta]
pub fn set_theme(state: State<'_, AppState>, theme: String) -> Result<(), String> {
    save_val(&state, KEY_THEME, &theme)
}

#[tauri::command]
#[specta::specta]
pub fn get_git_enabled(state: State<'_, AppState>) -> bool {
    get_val(&state, KEY_GIT_ENABLED)
        .map(|v| v == "true")
        .unwrap_or(false)
}

#[tauri::command]
#[specta::specta]
pub fn set_git_enabled(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    save_val(&state, KEY_GIT_ENABLED, if enabled { "true" } else { "false" })
}

#[tauri::command]
#[specta::specta]
pub fn get_git_scan_path(state: State<'_, AppState>) -> Option<String> {
    get_val(&state, KEY_GIT_SCAN_PATH)
}

#[tauri::command]
#[specta::specta]
pub fn set_git_scan_path(state: State<'_, AppState>, path: String) -> Result<(), String> {
    save_val(&state, KEY_GIT_SCAN_PATH, &path)
}

fn now() -> i64 {
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
