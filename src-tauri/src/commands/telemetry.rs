//! Frontend entry points for usage counters. Rust-side actions record
//! themselves directly via `crate::telemetry`; these commands exist for
//! interactions that only the UI sees (navigation clicks, view toggles,
//! export choices). Names are validated in `crate::telemetry::valid_key`.
use crate::state::AppState;
use tauri::State;

/// Increment the counter `name` (e.g. `nav.prev`). Fire-and-forget; invalid
/// names are dropped silently.
#[tauri::command]
#[specta::specta]
pub fn telemetry_track(state: State<'_, AppState>, name: String) {
    crate::telemetry::record(&state, &name);
}

/// Set a gauge (current state, not a count), e.g. `lang = "da"`.
#[tauri::command]
#[specta::specta]
pub fn telemetry_set_gauge(state: State<'_, AppState>, name: String, value: String) {
    crate::telemetry::set_gauge(&state, &name, &value);
}
