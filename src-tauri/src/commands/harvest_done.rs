// To-Do - this file is completely AI-coded, and not well-reviewed.
// We need to review it, and optimize it.
use std::collections::HashSet;

use rusqlite::{params, params_from_iter};
use tauri::State;
use uuid::{uuid, Uuid};

use crate::commands::settings;
use crate::state::AppState;

/// Fixed namespace for UUID v5 row keys derived from `TimelineEvent.id` (any source; id must be stable).
const HARVEST_DONE_NS: Uuid = uuid!("a312b8c4-d5e6-4789-b012-3456789abcde");

fn row_uuid_for_event_id(event_id: &str) -> String {
    Uuid::new_v5(&HARVEST_DONE_NS, event_id.as_bytes()).to_string()
}

#[tauri::command]
#[specta::specta]
pub fn get_timeline_harvest_done_for_event_ids(
    state: State<'_, AppState>,
    event_ids: Vec<String>,
) -> Result<Vec<String>, String> {
    if event_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut pairs: Vec<(String, String)> = Vec::with_capacity(event_ids.len());
    for event_id in event_ids {
        let row_id = row_uuid_for_event_id(&event_id);
        pairs.push((row_id, event_id));
    }

    let row_ids: Vec<String> = pairs.iter().map(|(r, _)| r.clone()).collect();

    let conn = state.db.lock().map_err(|_| "Failed to access database")?;

    let placeholders = (0..row_ids.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!("SELECT id FROM timeline_harvest_done WHERE id IN ({placeholders})");

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let found: HashSet<String> = stmt
        .query_map(params_from_iter(row_ids.iter()), |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    Ok(pairs
        .into_iter()
        .filter(|(row_id, _)| found.contains(row_id))
        .map(|(_, event_id)| event_id)
        .collect())
}

#[tauri::command]
#[specta::specta]
pub fn set_timeline_harvest_done(
    state: State<'_, AppState>,
    event_id: String,
    done: bool,
) -> Result<(), String> {
    let id = row_uuid_for_event_id(&event_id);
    let conn = state.db.lock().map_err(|_| "Failed to access database")?;

    if done {
        conn.execute(
            "INSERT OR REPLACE INTO timeline_harvest_done (id, updated_at) VALUES (?1, ?2)",
            params![id, settings::now()],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "DELETE FROM timeline_harvest_done WHERE id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{mock_app, state};

    fn get(app: &tauri::App<tauri::test::MockRuntime>, ids: &[&str]) -> Vec<String> {
        get_timeline_harvest_done_for_event_ids(
            state(app),
            ids.iter().map(|s| s.to_string()).collect(),
        )
        .unwrap()
    }

    fn set(app: &tauri::App<tauri::test::MockRuntime>, id: &str, done: bool) {
        set_timeline_harvest_done(state(app), id.to_string(), done).unwrap();
    }

    fn row_count(app: &tauri::App<tauri::test::MockRuntime>) -> i64 {
        let s = state(app);
        let conn = s.db.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM timeline_harvest_done", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn row_uuid_is_deterministic_and_distinct_per_event_id() {
        assert_eq!(row_uuid_for_event_id("git:/r:abc"), row_uuid_for_event_id("git:/r:abc"));
        assert_ne!(row_uuid_for_event_id("git:/r:abc"), row_uuid_for_event_id("git:/r:abd"));
    }

    #[test]
    fn row_uuid_is_a_version_5_uuid() {
        let parsed = Uuid::parse_str(&row_uuid_for_event_id("zulip:stream:x:2024-01-01")).unwrap();
        assert_eq!(parsed.get_version_num(), 5);
    }

    #[test]
    fn row_uuid_is_stable_across_releases() {
        // Rows on disk are keyed by this value; changing the namespace or the
        // hashing would orphan every existing checkmark.
        assert_eq!(
            row_uuid_for_event_id("git:/Users/me/code/recall:abc123"),
            Uuid::new_v5(&HARVEST_DONE_NS, b"git:/Users/me/code/recall:abc123").to_string()
        );
    }

    #[test]
    fn empty_input_returns_empty() {
        let app = mock_app();
        assert!(get(&app, &[]).is_empty());
    }

    #[test]
    fn unmarked_ids_are_not_returned() {
        let app = mock_app();
        assert!(get(&app, &["a", "b"]).is_empty());
    }

    #[test]
    fn marked_ids_come_back_in_input_order_and_only_those_asked_for() {
        let app = mock_app();
        set(&app, "b", true);
        set(&app, "c", true);
        set(&app, "z", true);
        assert_eq!(get(&app, &["a", "b", "c", "d"]), vec!["b", "c"]);
        assert_eq!(get(&app, &["c", "a", "b"]), vec!["c", "b"]);
        assert_eq!(get(&app, &["a"]), Vec::<String>::new());
    }

    #[test]
    fn marking_twice_is_idempotent() {
        let app = mock_app();
        set(&app, "a", true);
        set(&app, "a", true);
        assert_eq!(row_count(&app), 1);
        assert_eq!(get(&app, &["a"]), vec!["a"]);
    }

    #[test]
    fn unmarking_removes_the_row() {
        let app = mock_app();
        set(&app, "a", true);
        set(&app, "b", true);
        set(&app, "a", false);
        assert_eq!(get(&app, &["a", "b"]), vec!["b"]);
        assert_eq!(row_count(&app), 1);
    }

    #[test]
    fn unmarking_something_never_marked_is_ok() {
        let app = mock_app();
        set(&app, "ghost", false);
        assert_eq!(row_count(&app), 0);
    }

    #[test]
    fn rows_are_stored_under_the_hashed_id_not_the_raw_event_id() {
        let app = mock_app();
        set(&app, "git:/x:abc", true);
        let s = state(&app);
        let conn = s.db.lock().unwrap();
        let stored: String = conn
            .query_row("SELECT id FROM timeline_harvest_done", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stored, row_uuid_for_event_id("git:/x:abc"));
        assert_ne!(stored, "git:/x:abc");
    }

    #[test]
    fn ids_with_quotes_commas_and_unicode_are_handled() {
        let app = mock_app();
        let ids = ["it's", "a,b", "jira:DDF-1:2024-01-01:Kommentér", "x\"y"];
        for id in ids {
            set(&app, id, true);
        }
        assert_eq!(get(&app, &ids), ids.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    }
}
