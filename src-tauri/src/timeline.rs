use serde::{Deserialize, Serialize};
use specta::Type;

/// Return the input only if it is an http(s) URL; otherwise None.
/// Used to sanitize event URLs before they reach the frontend's openUrl().
/// Event URLs come from external feeds (iCal URL:, GitHub/Jira/Zulip API
/// responses); we refuse anything that could launch a non-http handler.
pub fn sanitize_event_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Activity source for the day timeline (matches frontend `TimelineEvent` styling keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum TimelineEventSource {
    Git,
    Github,
    Calendar,
    Gmail,
    Drive,
    Jira,
    Zulip,
}

/// One row on the timeline (all sources normalize to this shape).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TimelineEvent {
    /// Stable id for UI state (e.g. Harvest toggles). Must be unique per logical activity and
    /// identical across fetches/days — used for DB-backed toggles. Prefer a namespaced string
    /// (`git:…`, `gmail:…`, `zulip:…`) so ids never collide across sources.
    pub id: String,
    /// Local time `HH:MM` for the selected calendar day.
    pub time: String,
    pub source: TimelineEventSource,
    pub title: String,
    pub detail: Option<String>,
    pub url: Option<String>,
}
