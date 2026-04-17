use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;
use crate::commands::settings::{get_val, save_val};
use crate::state::AppState;

const KEY: &str = "settings_ui";

#[derive(Debug, Deserialize, Serialize, Type)]
#[specta(export = false)]
pub struct SettingsUi {
    pub theme: String,
}

#[tauri::command]
#[specta::specta]
pub fn set_settings_ui(
    state: State<'_, AppState>,
    settings: SettingsUi
) -> Result<(), String> {
    let json_data = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    save_val(&state, KEY, &json_data)
}

#[tauri::command]
#[specta::specta]
pub fn get_settings_ui(state: State<'_, AppState>) -> Option<SettingsUi> {
    let json_result = get_val(&state, KEY)?;
    serde_json::from_str(&json_result).ok()
}
