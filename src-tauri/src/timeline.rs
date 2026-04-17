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
    /// Stable id for UI state (e.g. Harvest toggles), e.g. `git:<repo>:<sha>`.
    pub id: String,
    /// Local time `HH:MM` for the selected calendar day.
    pub time: String,
    pub source: TimelineEventSource,
    pub title: String,
    pub detail: Option<String>,
    pub url: Option<String>,
}
