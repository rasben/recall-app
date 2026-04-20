use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub fn init_db(app_handle: &AppHandle) -> (Connection, PathBuf) {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .expect("failed to get app data dir");

    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).expect("failed to create app data dir");
    }

    let db_path = app_dir.join("db.sqlite");
    let conn = Connection::open(&db_path).expect("failed to open db");

    // WAL mode allows the background iCal sync (separate connection) to write
    // concurrently while the main connection reads the timeline.
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .expect("failed to enable WAL mode");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )
    .expect("failed to create settings table");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS timeline_harvest_done (
            id TEXT PRIMARY KEY,
            updated_at INTEGER NOT NULL
        )",
        [],
    )
    .expect("failed to create timeline_harvest_done table");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS timeline_day_cache (
            day TEXT PRIMARY KEY,
            events_json TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )
    .expect("failed to create timeline_day_cache table");

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
    .expect("failed to create ical_events table");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS ical_sync_meta (
            id INTEGER PRIMARY KEY,
            last_synced_at INTEGER,
            last_error TEXT
        )",
        [],
    )
    .expect("failed to create ical_sync_meta table");

    (conn, db_path)
}
