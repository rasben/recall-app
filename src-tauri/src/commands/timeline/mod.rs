mod git;
mod github;
mod jira;

use tauri::State;

use crate::state::AppState;
use crate::timeline::TimelineEvent;

#[tauri::command]
#[specta::specta]
pub fn get_timeline_for_day(
    state: State<'_, AppState>,
    day: String,
) -> Result<Vec<TimelineEvent>, String> {
    let mut rows: Vec<(i64, TimelineEvent)> = Vec::new();
    rows.extend(git::events_for_day(&state, &day)?);
    rows.extend(github::events_for_day(&state, &day)?);
    rows.extend(jira::events_for_day(&state, &day)?);
    rows.sort_by_key(|(ts, _)| *ts);
    Ok(rows.into_iter().map(|(_, ev)| ev).collect())
}
