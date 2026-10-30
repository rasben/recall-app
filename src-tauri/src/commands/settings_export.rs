use crate::commands::settings::{get_val, save_val};
use crate::state::AppState;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    load_settings_export(&state)
}

/// [`get_settings_export`] for callers holding a plain `&AppState` (the CLI)
/// rather than a Tauri `State`.
pub(crate) fn load_settings_export(state: &AppState) -> Option<SettingsExport> {
    let json_result = get_val(state, KEY)?;
    serde_json::from_str(&json_result).ok()
}

/// The built-in prompts by UI language. The frontend's translation table reads
/// the same file, so both sides agree on what "the default" is.
static DEFAULT_PROMPTS: Lazy<BTreeMap<String, String>> = Lazy::new(|| {
    serde_json::from_str(include_str!("../../../src/lib/export-prompts.json"))
        .expect("src/lib/export-prompts.json is a {lang: prompt} object")
});

/// Languages that have a built-in prompt (`en`, `da`).
pub(crate) fn prompt_languages() -> impl Iterator<Item = &'static str> {
    DEFAULT_PROMPTS.keys().map(String::as_str)
}

/// The built-in prompt for `lang`, or English when there is none.
pub(crate) fn default_export_prompt(lang: &str) -> &'static str {
    DEFAULT_PROMPTS
        .get(lang)
        .or_else(|| DEFAULT_PROMPTS.get("en"))
        .map(String::as_str)
        .unwrap_or_default()
}

/// The prompt an export should use, mirroring `resolveExportPrompt` in
/// `src/lib/export-prompt.ts`: the user's custom text, unless it is empty or
/// a built-in default (of any language), in which case `lang`'s default.
pub(crate) fn effective_export_prompt(state: &AppState, lang: &str) -> String {
    let stored = load_settings_export(state).map(|s| s.prompt).unwrap_or_default();
    let trimmed = stored.trim();
    if trimmed.is_empty() || DEFAULT_PROMPTS.values().any(|d| d.trim() == trimmed) {
        default_export_prompt(lang).to_string()
    } else {
        stored
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{mock_app, seed_setting, state};

    #[test]
    fn every_ui_language_has_a_distinct_default() {
        assert_eq!(prompt_languages().collect::<Vec<_>>(), vec!["da", "en"]);
        assert_ne!(default_export_prompt("en"), default_export_prompt("da"));
        assert!(default_export_prompt("en").contains("**Ticket/project**"));
    }

    #[test]
    fn unknown_languages_fall_back_to_english() {
        assert_eq!(default_export_prompt("sv"), default_export_prompt("en"));
    }

    #[test]
    fn nothing_stored_uses_the_requested_languages_default() {
        let app = mock_app();
        assert_eq!(effective_export_prompt(&state(&app), "da"), default_export_prompt("da"));
    }

    #[test]
    fn a_stored_default_follows_the_language_instead_of_pinning_it() {
        let app = mock_app();
        let stored = serde_json::json!({ "prompt": format!("{}\n", default_export_prompt("en")) });
        seed_setting(&state(&app), KEY, &stored.to_string());
        assert_eq!(effective_export_prompt(&state(&app), "da"), default_export_prompt("da"));
    }

    #[test]
    fn a_custom_prompt_is_used_verbatim() {
        let app = mock_app();
        seed_setting(&state(&app), KEY, r#"{"prompt":"Summarise my week."}"#);
        assert_eq!(effective_export_prompt(&state(&app), "da"), "Summarise my week.");
    }
}
