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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{event, mock_app, mock_app_at, state};
    use crate::timeline::{TimelineEvent, TimelineEventSource};

    fn seed_cache_row(state: &State<'_, AppState>, day: &str, events: &[TimelineEvent]) {
        let conn = state.db.lock().unwrap();
        conn.execute(
            "INSERT INTO timeline_day_cache (day, events_json, updated_at) VALUES (?1, ?2, ?3)",
            params![day, serde_json::to_string(events).unwrap(), now()],
        )
        .unwrap();
    }

    fn seed_ical_row(state: &State<'_, AppState>, uid: &str) {
        let conn = state.db.lock().unwrap();
        conn.execute(
            "INSERT INTO ical_events (url, uid, dtstart, summary) VALUES ('u', ?1, 1, 's')",
            params![uid],
        )
        .unwrap();
    }

    fn count(state: &State<'_, AppState>, table: &str) -> i64 {
        let conn = state.db.lock().unwrap();
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn now_is_milliseconds_since_epoch() {
        // 2020-01-01 in ms; anything smaller means seconds were returned.
        assert!(now() > 1_577_836_800_000);
    }

    #[test]
    fn get_val_is_none_for_unknown_key() {
        let app = mock_app();
        assert_eq!(get_val(&state(&app), "nope"), None);
    }

    #[test]
    fn save_val_roundtrips_and_overwrites() {
        let app = mock_app();
        let s = state(&app);
        save_val(&s, "k", "v1").unwrap();
        assert_eq!(get_val(&s, "k"), Some("v1".to_string()));
        save_val(&s, "k", "v2").unwrap();
        assert_eq!(get_val(&s, "k"), Some("v2".to_string()));
        assert_eq!(count(&s, "settings"), 1, "overwrite must not add rows");
    }

    #[test]
    fn save_val_keys_are_independent() {
        let app = mock_app();
        let s = state(&app);
        save_val(&s, "a", "1").unwrap();
        save_val(&s, "b", "2").unwrap();
        assert_eq!(get_val(&s, "a"), Some("1".to_string()));
        assert_eq!(get_val(&s, "b"), Some("2".to_string()));
    }

    #[test]
    fn cached_day_event_counts_is_empty_without_cache() {
        let app = mock_app();
        assert!(get_cached_day_event_counts(state(&app)).unwrap().is_empty());
    }

    #[test]
    fn cached_day_event_counts_reflect_the_stored_arrays() {
        let app = mock_app();
        let s = state(&app);
        seed_cache_row(&s, "2024-03-05", &[]);
        seed_cache_row(
            &s,
            "2024-03-06",
            &[
                event("a", 1, TimelineEventSource::Git),
                event("b", 2, TimelineEventSource::Jira),
                event("c", 3, TimelineEventSource::Zulip),
            ],
        );
        let counts = get_cached_day_event_counts(s).unwrap();
        assert_eq!(counts.len(), 2);
        assert_eq!(counts["2024-03-05"], 0);
        assert_eq!(counts["2024-03-06"], 3);
    }

    #[test]
    fn cache_size_counts_days_and_tolerates_a_missing_db_file() {
        let app = mock_app();
        let s = state(&app);
        seed_cache_row(&s, "2024-03-05", &[]);
        seed_cache_row(&s, "2024-03-06", &[]);
        let size = get_cache_size(s).unwrap();
        assert_eq!(size.cached_days, 2);
        assert_eq!(size.bytes, 0);
    }

    #[test]
    fn cache_size_reports_the_on_disk_size() {
        let path = std::env::temp_dir().join(format!("recall-test-{}.sqlite", uuid::Uuid::new_v4()));
        fs::write(&path, vec![0u8; 1234]).unwrap();
        let app = mock_app_at(path.clone());
        let size = get_cache_size(state(&app)).unwrap();
        let _ = fs::remove_file(&path);
        assert_eq!(size.bytes, 1234);
        assert_eq!(size.cached_days, 0);
    }

    #[test]
    fn clear_all_caches_on_an_empty_db_deletes_nothing() {
        let app = mock_app();
        assert_eq!(clear_all_caches(state(&app)).unwrap().rows_deleted, 0);
    }

    #[test]
    fn clear_all_caches_deletes_day_cache_and_ical_rows_only() {
        let app = mock_app();
        let s = state(&app);
        seed_cache_row(&s, "2024-03-05", &[]);
        seed_cache_row(&s, "2024-03-06", &[]);
        seed_ical_row(&s, "e1");
        seed_ical_row(&s, "e2");
        seed_ical_row(&s, "e3");
        save_val(&s, "settings_ui", r#"{"theme":"dark"}"#).unwrap();
        {
            let conn = s.db.lock().unwrap();
            conn.execute(
                "INSERT INTO timeline_harvest_done (id, updated_at) VALUES ('row', 1)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO ical_sync_meta (id, last_synced_at, last_error) VALUES (1, 5, NULL)",
                [],
            )
            .unwrap();
        }

        let result = clear_all_caches(state(&app)).unwrap();
        assert_eq!(result.rows_deleted, 5);
        assert_eq!(count(&s, "timeline_day_cache"), 0);
        assert_eq!(count(&s, "ical_events"), 0);
        // User data and sync metadata are not caches.
        assert_eq!(get_val(&s, "settings_ui"), Some(r#"{"theme":"dark"}"#.to_string()));
        assert_eq!(count(&s, "timeline_harvest_done"), 1);
        assert_eq!(count(&s, "ical_sync_meta"), 1);
    }
}
