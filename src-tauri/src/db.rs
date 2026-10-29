use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// Must equal `identifier` in `tauri.conf.json`: Tauri names the app data
/// folder after it, and the CLI has no `AppHandle` to ask.
pub(crate) const APP_IDENTIFIER: &str = "com.recall-app.app";

/// The app's `db.sqlite`, resolved the way Tauri's `app_data_dir` does
/// (`dirs::data_dir()/{identifier}`), for callers without an `AppHandle`.
pub(crate) fn default_db_path() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|dir| dir.join(APP_IDENTIFIER).join("db.sqlite"))
        .ok_or_else(|| "failed to resolve the user data directory".to_string())
}

/// Open a database the app has already created. Unlike `init_db` this never
/// creates one: a missing file means the app was never set up on this machine.
pub(crate) fn open_existing(db_path: &Path) -> Result<Connection, String> {
    if !db_path.is_file() {
        return Err(format!(
            "no Recall database at {} (open the Recall app and configure a data source first)",
            db_path.display()
        ));
    }
    let conn = Connection::open(db_path)
        .map_err(|e| format!("failed to open db at {}: {e}", db_path.display()))?;
    init_schema(&conn)?;
    Ok(conn)
}

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

    init_schema(&conn)?;

    Ok((conn, db_path))
}

/// Create every table (idempotent) and apply the column migrations. Split out
/// of `init_db` so tests can run the production schema against an in-memory
/// connection.
pub(crate) fn init_schema(conn: &Connection) -> Result<(), String> {
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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS telemetry_counters (
            key TEXT PRIMARY KEY,
            value INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )
    .map_err(|e| format!("failed to create telemetry_counters table: {e}"))?;

    // Migrations: ignore errors when columns already exist.
    let _ = conn.execute("ALTER TABLE ical_events ADD COLUMN dtend INTEGER", []);
    let _ = conn.execute(
        "ALTER TABLE ical_events ADD COLUMN declined INTEGER NOT NULL DEFAULT 0",
        [],
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CLI finds the app's DB through `APP_IDENTIFIER`; if it drifts from
    /// the Tauri config the CLI would silently read the wrong (absent) DB.
    #[test]
    fn app_identifier_matches_tauri_config() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], APP_IDENTIFIER);
        assert!(
            conf["app"].get("appDirectoriesOverride").is_none(),
            "default_db_path assumes Tauri's default app data dir"
        );
    }

    #[test]
    fn default_db_path_lives_in_the_identifier_folder() {
        let path = default_db_path().unwrap();
        assert!(path.ends_with(format!("{APP_IDENTIFIER}/db.sqlite")));
    }

    #[test]
    fn open_existing_refuses_to_create_a_database() {
        let dir = std::env::temp_dir().join(format!("recall-missing-{}", uuid::Uuid::new_v4()));
        let err = open_existing(&dir.join("db.sqlite")).unwrap_err();
        assert!(err.contains("no Recall database"), "{err}");
        assert!(!dir.exists());
    }

    fn table_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
                 ORDER BY name",
            )
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    fn columns(conn: &Connection, table: &str) -> Vec<String> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})")).unwrap();
        stmt.query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn creates_every_table() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        assert_eq!(
            table_names(&conn),
            vec![
                "ical_events",
                "ical_sync_meta",
                "settings",
                "telemetry_counters",
                "timeline_day_cache",
                "timeline_harvest_done",
            ]
        );
    }

    #[test]
    fn is_idempotent_and_keeps_existing_rows() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES ('k', 'v', 1)",
            [],
        )
        .unwrap();
        init_schema(&conn).expect("second init must not fail");
        let v: String = conn
            .query_row("SELECT value FROM settings WHERE key = 'k'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "v");
        assert_eq!(table_names(&conn).len(), 6);
    }

    #[test]
    fn ical_events_has_the_migrated_columns() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let cols = columns(&conn, "ical_events");
        for c in ["url", "uid", "dtstart", "dtend", "summary", "event_url", "declined"] {
            assert!(cols.iter().any(|x| x == c), "missing column {c} in {cols:?}");
        }
    }

    #[test]
    fn migrates_a_pre_migration_ical_events_table() {
        // A DB created before `dtend` / `declined` existed must gain both
        // columns, with existing rows defaulting to not-declined.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE ical_events (
                url TEXT NOT NULL,
                uid TEXT NOT NULL,
                dtstart INTEGER NOT NULL,
                summary TEXT NOT NULL,
                event_url TEXT,
                PRIMARY KEY (url, uid, dtstart)
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ical_events (url, uid, dtstart, summary) VALUES ('u', 'id', 1, 's')",
            [],
        )
        .unwrap();

        init_schema(&conn).unwrap();

        let (dtend, declined): (Option<i64>, i64) = conn
            .query_row("SELECT dtend, declined FROM ical_events", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(dtend, None);
        assert_eq!(declined, 0);
    }

    #[test]
    fn primary_keys_enforce_uniqueness() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO timeline_day_cache (day, events_json, updated_at) VALUES ('2024-01-01', '[]', 1)",
            [],
        )
        .unwrap();
        let dup = conn.execute(
            "INSERT INTO timeline_day_cache (day, events_json, updated_at) VALUES ('2024-01-01', '[]', 2)",
            [],
        );
        assert!(dup.is_err(), "day must be unique in timeline_day_cache");

        conn.execute(
            "INSERT INTO ical_events (url, uid, dtstart, summary) VALUES ('u', 'id', 1, 's')",
            [],
        )
        .unwrap();
        let dup = conn.execute(
            "INSERT INTO ical_events (url, uid, dtstart, summary) VALUES ('u', 'id', 1, 'other')",
            [],
        );
        assert!(dup.is_err(), "(url, uid, dtstart) must be unique in ical_events");
    }
}
