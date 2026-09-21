use crate::commands::settings::{get_val, save_val};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

const KEY: &str = "settings_ui";

fn default_group_mode() -> String {
    "time".to_string()
}

#[derive(Debug, Deserialize, Serialize, Type)]
#[specta(export = false)]
pub struct SettingsUi {
    pub theme: String,
    /// How the day's events are grouped: `"time"` or `"task"`. Stored as a
    /// plain string (the frontend validates it) so an unknown value can never
    /// make the whole document fail to load.
    #[serde(default = "default_group_mode")]
    pub group_mode: String,
    /// Timeline sources the user has toggled off (`TimelineEventSource` keys,
    /// e.g. `"zulip"`). Plain strings for the same reason as `group_mode`.
    #[serde(default)]
    pub hidden_sources: Vec<String>,
}

#[tauri::command]
#[specta::specta]
pub fn set_settings_ui(state: State<'_, AppState>, settings: SettingsUi) -> Result<(), String> {
    let json_data = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    save_val(&state, KEY, &json_data)
}

#[tauri::command]
#[specta::specta]
pub fn get_settings_ui(state: State<'_, AppState>) -> Option<SettingsUi> {
    let json_result = get_val(&state, KEY)?;
    serde_json::from_str(&json_result).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::settings::get_val;
    use crate::test_support::{mock_app, seed_setting, state};

    fn ui(theme: &str) -> SettingsUi {
        SettingsUi {
            theme: theme.into(),
            group_mode: default_group_mode(),
            hidden_sources: Vec::new(),
        }
    }

    #[test]
    fn get_is_none_when_nothing_is_saved() {
        let app = mock_app();
        assert!(get_settings_ui(state(&app)).is_none());
    }

    #[test]
    fn set_then_get_roundtrips() {
        let app = mock_app();
        set_settings_ui(state(&app), ui("dark")).unwrap();
        let ui = get_settings_ui(state(&app)).expect("settings saved");
        assert_eq!(ui.theme, "dark");
    }

    #[test]
    fn set_overwrites_the_previous_document() {
        let app = mock_app();
        set_settings_ui(state(&app), ui("dark")).unwrap();
        set_settings_ui(state(&app), ui("light")).unwrap();
        assert_eq!(get_settings_ui(state(&app)).unwrap().theme, "light");
    }

    #[test]
    fn stores_one_json_document_under_the_domain_key() {
        let app = mock_app();
        set_settings_ui(state(&app), ui("system")).unwrap();
        let raw = get_val(&state(&app), KEY).expect("stored under settings_ui");
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(v.is_object());
        assert_eq!(v["theme"], "system");
    }

    #[test]
    fn a_theme_only_document_from_older_versions_still_loads() {
        // Existing installs have exactly this shape on disk; adding fields to
        // SettingsUi must not make it fail to parse (and silently reset).
        let app = mock_app();
        seed_setting(&state(&app), KEY, r#"{"theme":"light"}"#);
        let ui = get_settings_ui(state(&app)).expect("legacy document must load");
        assert_eq!(ui.theme, "light");
        assert_eq!(ui.group_mode, "time");
        assert!(ui.hidden_sources.is_empty());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let app = mock_app();
        seed_setting(&state(&app), KEY, r#"{"theme":"dark","from_the_future":42}"#);
        assert_eq!(get_settings_ui(state(&app)).unwrap().theme, "dark");
    }

    #[test]
    fn a_corrupt_document_yields_none_rather_than_an_error() {
        let app = mock_app();
        seed_setting(&state(&app), KEY, "not json at all");
        assert!(get_settings_ui(state(&app)).is_none());
    }

    #[test]
    fn view_state_round_trips() {
        let app = mock_app();
        let saved = SettingsUi {
            theme: "system".into(),
            group_mode: "task".into(),
            hidden_sources: vec!["zulip".into(), "git".into()],
        };
        set_settings_ui(state(&app), saved).unwrap();
        let back = get_settings_ui(state(&app)).expect("settings saved");
        assert_eq!(back.group_mode, "task");
        assert_eq!(back.hidden_sources, vec!["zulip", "git"]);
    }
}
