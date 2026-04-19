use crate::commands::settings::{get_val, save_val};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

const KEY: &str = "settings_zulip";

fn default_realm_url() -> String {
    "https://reload.zulipchat.com".to_string()
}

#[derive(Deserialize, Serialize, Type)]
#[specta(export = false)]
pub struct SettingsZulip {
    pub enabled: bool,
    #[serde(default = "default_realm_url")]
    pub realm_url: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub api_key: String,
}

#[tauri::command]
#[specta::specta]
pub fn set_settings_zulip(
    state: State<'_, AppState>,
    settings: SettingsZulip,
) -> Result<(), String> {
    let json_data = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    save_val(&state, KEY, &json_data)
}

#[tauri::command]
#[specta::specta]
pub fn get_settings_zulip(state: State<'_, AppState>) -> Option<SettingsZulip> {
    let json = get_val(&state, KEY)?;
    serde_json::from_str(&json).ok()
}
