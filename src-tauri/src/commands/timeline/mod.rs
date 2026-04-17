mod git;

use tauri::State;

use crate::state::AppState;
use crate::timeline::TimelineEvent;

#[tauri::command]
#[specta::specta]
pub fn get_timeline_for_day(
    state: State<'_, AppState>,
    day: String,
) -> Result<Vec<TimelineEvent>, String> {
    // Additional sources (e.g. Zulip) extend here, then merge/sort across sources if needed.
    git::events_for_day(&state, &day)
}
