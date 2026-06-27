use crate::commands::settings::{get_val, save_val};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

const KEY: &str = "settings_export";

#[derive(Debug, Deserialize, Serialize, Type)]
#[specta(export = false)]
pub struct SettingsExport {
    /// User's custom prompt intro for the AI export. An empty string means
    /// "use the app's built-in, language-aware default", which the frontend
    /// resolves against the active UI language.
    pub prompt: String,
}

#[tauri::command]
#[specta::specta]
pub fn set_settings_export(state: State<'_, AppState>, settings: SettingsExport) -> Result<(), String> {
    let json_data = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    save_val(&state, KEY, &json_data)
}

#[tauri::command]
#[specta::specta]
pub fn get_settings_export(state: State<'_, AppState>) -> Option<SettingsExport> {
    let json_result = get_val(&state, KEY)?;
    serde_json::from_str(&json_result).ok()
}
