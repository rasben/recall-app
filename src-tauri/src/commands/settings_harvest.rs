use crate::commands::harvest::fetch_current_user;
use crate::commands::settings::{get_val, save_val};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

const KEY: &str = "settings_harvest";

/// Harvest API v2 credentials: a personal access token plus the numeric
/// account id it belongs to (both from https://id.getharvest.com/developers).
#[derive(Deserialize, Serialize, Type)]
#[specta(export = false)]
pub struct SettingsHarvest {
    pub enabled: bool,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub account_id: String,
}

impl SettingsHarvest {
    /// `Some((token, account_id))` when the source is enabled and both
    /// credentials are present, otherwise `None`.
    pub fn credentials(&self) -> Option<(&str, &str)> {
        if !self.enabled {
            return None;
        }
        let token = self.access_token.trim();
        let account_id = self.account_id.trim();
        if token.is_empty() || account_id.is_empty() {
            return None;
        }
        Some((token, account_id))
    }
}

#[tauri::command]
#[specta::specta]
pub fn set_settings_harvest(
    state: State<'_, AppState>,
    settings: SettingsHarvest,
) -> Result<(), String> {
    let json_data = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    save_val(&state, KEY, &json_data)
}

#[tauri::command]
#[specta::specta]
pub fn get_settings_harvest(state: State<'_, AppState>) -> Option<SettingsHarvest> {
    let json = get_val(&state, KEY)?;
    serde_json::from_str(&json).ok()
}

/// Verify the saved token + account id by calling `GET /v2/users/me`.
#[tauri::command]
#[specta::specta]
pub async fn test_settings_harvest(state: State<'_, AppState>) -> Result<(), String> {
    tokio::task::block_in_place(|| {
        let Some(settings) = get_settings_harvest(state.clone()) else {
            return Err("Harvest is not configured".into());
        };
        if !settings.enabled {
            return Err("Harvest is not enabled".into());
        }
        if settings.access_token.trim().is_empty() || settings.account_id.trim().is_empty() {
            return Err("Access token and account ID are required".into());
        }
        fetch_current_user(settings.access_token.trim(), settings.account_id.trim()).map(|_| ())
    })
}
