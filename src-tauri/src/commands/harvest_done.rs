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
