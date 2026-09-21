use crate::commands::settings::{get_val, save_val};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

pub(crate) const KEY: &str = "settings_telemetry";

#[derive(Debug, Deserialize, Serialize, Type)]
#[specta(export = false)]
pub struct SettingsTelemetry {
    /// Anonymous daily usage summary on/off. Defaults to on; see `telemetry.rs`.
    pub enabled: bool,
}

impl Default for SettingsTelemetry {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[tauri::command]
#[specta::specta]
pub fn get_settings_telemetry(state: State<'_, AppState>) -> SettingsTelemetry {
    get_val(&state, KEY)
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

#[tauri::command]
#[specta::specta]
pub fn set_settings_telemetry(
    state: State<'_, AppState>,
    settings: SettingsTelemetry,
) -> Result<(), String> {
    let json_data = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    save_val(&state, KEY, &json_data)?;
    if !settings.enabled {
        // Opting out also discards whatever was counted but not yet sent.
        let conn = state.db.lock().map_err(|_| "Failed to access database")?;
        conn.execute("DELETE FROM telemetry_counters", [])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
