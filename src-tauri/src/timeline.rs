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
    /// UTC unix seconds. Used by the UI to group close-together events
    /// (e.g. commits inside a rebase burst) at finer resolution than `time`.
    pub timestamp: i64,
    pub source: TimelineEventSource,
    pub title: String,
    pub detail: Option<String>,
    pub url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_accepts_http_and_https() {
        assert_eq!(
            sanitize_event_url("https://example.com/x?y=1#z"),
            Some("https://example.com/x?y=1#z".to_string())
        );
        assert_eq!(
            sanitize_event_url("http://example.com"),
            Some("http://example.com".to_string())
        );
    }

    #[test]
    fn sanitize_matches_scheme_case_insensitively_but_keeps_original() {
        assert_eq!(
            sanitize_event_url("HTTPS://Example.com/A"),
            Some("HTTPS://Example.com/A".to_string())
        );
    }

    #[test]
    fn sanitize_trims_surrounding_whitespace() {
        assert_eq!(
            sanitize_event_url("  https://a.b/c \n"),
            Some("https://a.b/c".to_string())
        );
    }

    #[test]
    fn sanitize_rejects_empty_and_whitespace_only() {
        assert_eq!(sanitize_event_url(""), None);
        assert_eq!(sanitize_event_url("   \t"), None);
    }

    #[test]
    fn sanitize_rejects_non_http_schemes_and_bare_hosts() {
        for raw in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "mailto:someone@example.com",
            "ftp://example.com/x",
            "//evil.example.com",
            "example.com",
            "http:/missing-slash",
            "https:example.com",
            " javascript:https://looks-legit",
        ] {
            assert_eq!(sanitize_event_url(raw), None, "should reject {raw:?}");
        }
    }

    #[test]
    fn source_serializes_as_lowercase_frontend_keys() {
        let cases = [
            (TimelineEventSource::Git, "git"),
            (TimelineEventSource::Github, "github"),
            (TimelineEventSource::Calendar, "calendar"),
            (TimelineEventSource::Gmail, "gmail"),
            (TimelineEventSource::Drive, "drive"),
            (TimelineEventSource::Jira, "jira"),
            (TimelineEventSource::Zulip, "zulip"),
        ];
        for (source, key) in cases {
            let json = serde_json::to_string(&source).unwrap();
            assert_eq!(json, format!("\"{key}\""));
            let back: TimelineEventSource = serde_json::from_str(&json).unwrap();
            assert_eq!(back, source);
        }
    }

    #[test]
    fn event_json_roundtrip_preserves_every_field() {
        let ev = TimelineEvent {
            id: "git:/repo:abc".into(),
            time: "09:30".into(),
            timestamp: 1_700_000_000,
            source: TimelineEventSource::Git,
            title: "Fix bug".into(),
            detail: Some("recall — abc1234".into()),
            url: Some("https://github.com/x/y/commit/abc".into()),
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: TimelineEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, ev.id);
        assert_eq!(back.time, ev.time);
        assert_eq!(back.timestamp, ev.timestamp);
        assert_eq!(back.source, ev.source);
        assert_eq!(back.title, ev.title);
        assert_eq!(back.detail, ev.detail);
        assert_eq!(back.url, ev.url);
    }

    #[test]
    fn event_json_keeps_null_optionals_as_null() {
        // The cache stores this JSON verbatim, so the shape must stay stable.
        let ev = TimelineEvent {
            id: "x".into(),
            time: "00:00".into(),
            timestamp: 0,
            source: TimelineEventSource::Zulip,
            title: "t".into(),
            detail: None,
            url: None,
        };
        let v: serde_json::Value = serde_json::to_value(&ev).unwrap();
        assert!(v.get("detail").unwrap().is_null());
        assert!(v.get("url").unwrap().is_null());
        assert_eq!(v.get("source").unwrap(), "zulip");
    }
}
