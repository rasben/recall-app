use rusqlite::Connection;
use std::fs;
use tauri::{AppHandle, Manager};

pub fn init_db(app_handle: &AppHandle) -> Connection {
    let app_dir = app_handle.path().app_data_dir().expect("failed to get app data dir");

    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).expect("failed to create app data dir");
    }

    let db_path = app_dir.join("db.sqlite");
    let conn = Connection::open(db_path).expect("failed to open db");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    ).expect("failed to create settings table");

    conn
}
