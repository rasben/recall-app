use serde::{Deserialize, Serialize};
use specta::Type;

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
