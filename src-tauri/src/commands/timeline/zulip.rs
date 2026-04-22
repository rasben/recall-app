use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use tauri::State;

use crate::commands::settings_zulip::get_settings_zulip;
use crate::state::AppState;
use crate::timeline::{TimelineEvent, TimelineEventSource};

fn normalize_realm_url(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_string()
}

pub(super) fn events_for_day(
    state: &State<'_, AppState>,
    day: &str,
) -> Result<Vec<(i64, TimelineEvent)>, String> {
    let Some(settings) = get_settings_zulip(state.clone()) else {
        return Ok(Vec::new());
    };
    if !settings.enabled {
        return Ok(Vec::new());
    }
    let realm = normalize_realm_url(&settings.realm_url);
    if realm.is_empty() || settings.email.trim().is_empty() || settings.api_key.trim().is_empty() {
        return Ok(Vec::new());
    }

    let email = settings.email.trim().to_string();
    let api_key = settings.api_key.trim().to_string();

    let day_naive =
        NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|_| format!("Invalid date: {day}"))?;
    let next_day = day_naive
        .succ_opt()
        .ok_or_else(|| format!("no day after {day}"))?;

    let day_start = Local
        .from_local_datetime(&day_naive.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .map(|d| d.timestamp())
        .unwrap_or(0);
    let day_end = Local
        .from_local_datetime(&next_day.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .map(|d| d.timestamp())
        .unwrap_or(i64::MAX);

    let narrow = serde_json::json!([{"operator": "sender", "operand": email}]);
    let narrow_str =
        serde_json::to_string(&narrow).map_err(|e| format!("Zulip narrow JSON: {e}"))?;

    let fetch_url = format!(
        "{realm}/api/v1/messages?anchor=newest&num_before=5000&num_after=0&narrow={}&apply_markdown=false",
        urlencoding::encode(&narrow_str)
    );

    let auth = format!("Basic {}", STANDARD.encode(format!("{email}:{api_key}")));
    let resp = ureq::get(&fetch_url)
        .set("Authorization", &auth)
        .set("Accept", "application/json")
        .call();

    let (status, body) = match resp {
        Ok(r) => {
            let status = r.status();
            let body = r
                .into_string()
                .map_err(|e| format!("Zulip read body: {e}"))?;
            (status, body)
        }
        Err(ureq::Error::Status(status, r)) => {
            let body = r.into_string().unwrap_or_default();
            (status, body)
        }
        Err(e) => return Err(format!("Zulip HTTP: {e}")),
    };

    if status >= 400 {
        return Ok(Vec::new());
    }

    let parsed: MessagesResponse =
        serde_json::from_str(&body).map_err(|e| format!("Zulip messages JSON: {e}"))?;

    // Filter to the requested day and group by stream (or DM bucket).
    let mut groups: HashMap<String, Vec<ZulipMessage>> = HashMap::new();
    for msg in parsed.messages {
        if msg.timestamp < day_start || msg.timestamp >= day_end {
            continue;
        }
        let key = if msg.msg_type == "stream" {
            msg.display_recipient
                .as_str()
                .unwrap_or("unknown")
                .to_string()
        } else {
            "__dm__".to_string()
        };
        groups.entry(key).or_default().push(msg);
    }

    let mut rows: Vec<(i64, TimelineEvent)> = Vec::new();
    for (stream_key, mut msgs) in groups {
        msgs.sort_by_key(|m| m.timestamp);
        let earliest_ts = msgs[0].timestamp;
        let count = msgs.len();
        let is_dm = stream_key == "__dm__";

        let dt = DateTime::<Utc>::from_timestamp(earliest_ts, 0)
            .ok_or_else(|| format!("invalid timestamp {earliest_ts}"))?
            .with_timezone(&Local);
        let time = dt.format("%H:%M").to_string();

        let noun = if count == 1 { "message" } else { "messages" };
        let title = if is_dm {
            format!("Sent {count} direct {noun}")
        } else {
            format!("Sent {count} {noun} in #{stream_key}")
        };

        // detail: deduplicated topic names (shown always, one line).
        let mut seen_topics: Vec<&str> = Vec::new();
        for m in &msgs {
            let t = m.subject.as_deref().unwrap_or("(no topic)");
            if !seen_topics.contains(&t) {
                seen_topics.push(t);
            }
        }
        let detail = seen_topics.join(", ");


        let first_id = msgs[0].id;
        let first_topic = msgs[0].subject.as_deref().unwrap_or("");
        let msg_url = if is_dm {
            format!("{realm}/#narrow/near/{first_id}")
        } else {
            let stream_segment = match msgs[0].stream_id {
                Some(sid) => format!("{}-{}", sid, zulip_encode(&stream_key)),
                None => zulip_encode(&stream_key),
            };
            format!(
                "{realm}/#narrow/stream/{stream_segment}/topic/{}/near/{first_id}",
                zulip_encode(first_topic)
            )
        };

        let id = if is_dm {
            format!("zulip:dm:{day}")
        } else {
            format!("zulip:stream:{}:{day}", stream_key)
        };

        rows.push((
            earliest_ts,
            TimelineEvent {
                id,
                time,
                source: TimelineEventSource::Zulip,
                title,
                detail: Some(detail),
                url: Some(msg_url),
            },
        ));
    }

    rows.sort_by_key(|(ts, _)| *ts);
    Ok(rows)
}


#[derive(Deserialize)]
struct MessagesResponse {
    messages: Vec<ZulipMessage>,
}

/// Zulip's narrow-component encoding: encodeURIComponent but with '%' replaced by '.', lowercased.
fn zulip_encode(s: &str) -> String {
    let encoded = urlencoding::encode(s);
    encoded.replace('%', ".").to_lowercase()
}

#[derive(Deserialize)]
struct ZulipMessage {
    id: u64,
    timestamp: i64,
    #[serde(rename = "type")]
    msg_type: String,
    #[serde(default)]
    subject: Option<String>,
    display_recipient: serde_json::Value,
    #[serde(default)]
    stream_id: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- pure helpers ---

    #[test]
    fn zulip_encode_plain_text() {
        assert_eq!(zulip_encode("hello"), "hello");
    }

    #[test]
    fn zulip_encode_space() {
        // space → %20 → .20
        assert_eq!(zulip_encode("hello world"), "hello.20world");
    }

    #[test]
    fn zulip_encode_special_chars() {
        // # → %23 → .23
        assert_eq!(zulip_encode("C++"), "c.2b.2b");
    }

    #[test]
    fn normalize_realm_url_strips_trailing_slash() {
        assert_eq!(
            normalize_realm_url("https://example.zulipchat.com/"),
            "https://example.zulipchat.com"
        );
    }

    #[test]
    fn normalize_realm_url_unchanged_when_clean() {
        assert_eq!(
            normalize_realm_url("https://example.zulipchat.com"),
            "https://example.zulipchat.com"
        );
    }

    // --- integration (skipped when secrets absent) ---

    #[test]
    fn zulip_api_reachable_with_valid_credentials() {
        let realm = match std::env::var("RECALL_TEST_ZULIP_REALM_URL") {
            Ok(r) => r,
            Err(_) => return,
        };
        let email = match std::env::var("RECALL_TEST_ZULIP_EMAIL") {
            Ok(e) => e,
            Err(_) => return,
        };
        let api_key = match std::env::var("RECALL_TEST_ZULIP_API_KEY") {
            Ok(k) => k,
            Err(_) => return,
        };

        let realm = normalize_realm_url(&realm);
        let url = format!("{realm}/api/v1/users/me");
        let auth = format!("Basic {}", STANDARD.encode(format!("{email}:{api_key}")));

        let resp = ureq::get(&url)
            .set("Authorization", &auth)
            .set("Accept", "application/json")
            .call()
            .expect("Zulip API request failed");

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = resp.into_json().expect("Valid JSON from Zulip /users/me");
        assert_eq!(
            body.get("result").and_then(|v| v.as_str()),
            Some("success"),
            "Zulip /users/me result was not 'success': {body}"
        );
    }
}
