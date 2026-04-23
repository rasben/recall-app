use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub fn init_db(app_handle: &AppHandle) -> Result<(Connection, PathBuf), String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

    if !app_dir.exists() {
        fs::create_dir_all(&app_dir)
            .map_err(|e| format!("failed to create app data dir {}: {e}", app_dir.display()))?;
    }

    let db_path = app_dir.join("db.sqlite");
    let conn = Connection::open(&db_path)
        .map_err(|e| format!("failed to open db at {}: {e}", db_path.display()))?;

    // WAL mode allows the background iCal sync (separate connection) to write
    // concurrently while the main connection reads the timeline.
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|e| format!("failed to enable WAL mode: {e}"))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("failed to create settings table: {e}"))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS timeline_harvest_done (
            id TEXT PRIMARY KEY,
            updated_at INTEGER NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("failed to create timeline_harvest_done table: {e}"))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS timeline_day_cache (
            day TEXT PRIMARY KEY,
            events_json TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("failed to create timeline_day_cache table: {e}"))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS ical_events (
            url TEXT NOT NULL,
            uid TEXT NOT NULL,
            dtstart INTEGER NOT NULL,
            summary TEXT NOT NULL,
            event_url TEXT,
            PRIMARY KEY (url, uid, dtstart)
        )",
        [],
    )
    .map_err(|e| format!("failed to create ical_events table: {e}"))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS ical_sync_meta (
            id INTEGER PRIMARY KEY,
            last_synced_at INTEGER,
            last_error TEXT
        )",
        [],
    )
    .map_err(|e| format!("failed to create ical_sync_meta table: {e}"))?;

    // Migrations: ignore errors when columns already exist.
    let _ = conn.execute("ALTER TABLE ical_events ADD COLUMN dtend INTEGER", []);
    let _ = conn.execute(
        "ALTER TABLE ical_events ADD COLUMN declined INTEGER NOT NULL DEFAULT 0",
        [],
    );

    Ok((conn, db_path))
}
