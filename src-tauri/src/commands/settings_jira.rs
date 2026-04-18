use crate::commands::settings::{get_val, save_val};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

const KEY: &str = "settings_jira";

fn default_site_url() -> String {
    "https://reload.atlassian.net".to_string()
}

fn default_jira_events() -> Vec<JiraEvent> {
    vec![
        JiraEvent::CommentWritten,
        JiraEvent::IssueCreated,
        JiraEvent::IssueCompleted,
        JiraEvent::Mentioned,
    ]
}

/// Timeline categories for Jira activity (mapped to REST/changelog rules in the Jira timeline source).
#[derive(Clone, Copy, Deserialize, Serialize, Type)]
#[specta(export = false)]
#[derive(PartialEq, Eq)]
pub enum JiraEvent {
    /// Comments you authored on issues.
    CommentWritten,
    /// Issues where you are the reporter/creator.
    IssueCreated,
    /// Issues transitioned into a Done-style status category.
    IssueCompleted,
    /// You were @mentioned in an issue field or comment body.
    Mentioned,
}

#[derive(Deserialize, Serialize, Type)]
#[specta(export = false)]
pub struct SettingsJira {
    pub enabled: bool,
    #[serde(default = "default_site_url")]
    pub site_url: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub api_token: String,
    #[serde(default = "default_jira_events")]
    pub enabled_events: Vec<JiraEvent>,
}

#[tauri::command]
#[specta::specta]
pub fn set_settings_jira(state: State<'_, AppState>, settings: SettingsJira) -> Result<(), String> {
    let json_data = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    save_val(&state, KEY, &json_data)
}

#[tauri::command]
#[specta::specta]
pub fn get_settings_jira(state: State<'_, AppState>) -> Option<SettingsJira> {
    let json = get_val(&state, KEY)?;
    serde_json::from_str(&json).ok()
}
