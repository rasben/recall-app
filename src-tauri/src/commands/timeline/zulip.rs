use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use tauri::State;

/// Upper bound on messages fetched per API call (Zulip server caps at ~1000).
const ZULIP_PAGE_SIZE: u32 = 1000;
/// Upper bound on pages per day lookup — prevents runaway pagination for users
/// who send huge volumes. 10 * 1000 = 10k messages is far more than a realistic
/// single day, and if we don't reach the target day by then, something is wrong.
const ZULIP_MAX_PAGES: u32 = 10;

use crate::commands::settings_zulip::get_settings_zulip;
use crate::state::AppState;
use crate::timeline::{sanitize_event_url, TimelineEvent, TimelineEventSource};

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

    let auth = format!("Basic {}", STANDARD.encode(format!("{email}:{api_key}")));

    // The /api/v1/messages endpoint has no date filter, so we page backwards
    // from "newest" until we either (a) see a message older than day_start
    // (meaning the day is fully covered) or (b) the server reports there are
    // no older messages. Without this loop an older day beyond the most recent
    // ~1000 messages would silently appear empty.
    let mut anchor: String = "newest".to_string();
    let mut seen_ids: HashSet<u64> = HashSet::new();
    let mut in_window: Vec<ZulipMessage> = Vec::new();

    for page_idx in 0..ZULIP_MAX_PAGES {
        let fetch_url = format!(
            "{realm}/api/v1/messages?anchor={}&num_before={}&num_after=0&narrow={}&apply_markdown=false",
            urlencoding::encode(&anchor),
            ZULIP_PAGE_SIZE,
            urlencoding::encode(&narrow_str)
        );

        let resp = ureq::get(&fetch_url)
            .header("Authorization", &auth)
            .header("Accept", "application/json")
            .call();

        let (status, body) = match resp {
            Ok(mut r) => {
                let status = r.status().as_u16();
                let body = r
                    .body_mut()
                    .read_to_string()
                    .map_err(|e| format!("Zulip read body: {e}"))?;
                (status, body)
            }
            Err(ureq::Error::StatusCode(status)) => (status, String::new()),
            Err(e) => return Err(format!("Zulip HTTP: {e}")),
        };

        if status >= 400 {
            return Err(format!(
                "Zulip returned HTTP {status} (check realm URL, email, and API key)"
            ));
        }

        let parsed: MessagesResponse = serde_json::from_str(&body)
            .map_err(|e| format!("Zulip messages JSON: {e}"))?;

        if parsed.messages.is_empty() {
            break;
        }

        let mut earliest_ts = i64::MAX;
        let mut earliest_id: Option<u64> = None;
        for msg in parsed.messages {
            if !seen_ids.insert(msg.id) {
                continue;
            }
            if msg.timestamp < earliest_ts {
                earliest_ts = msg.timestamp;
                earliest_id = Some(msg.id);
            }
            if msg.timestamp >= day_start && msg.timestamp < day_end {
                in_window.push(msg);
            }
        }

        // Reached the far side of the target day, or Zulip says there's
        // nothing older than what we just got.
        if earliest_ts < day_start || parsed.found_oldest.unwrap_or(false) {
            break;
        }

        let Some(next_anchor_id) = earliest_id else {
            break;
        };

        // Guard against an unexpected infinite loop (server returning
        // the same batch for the same anchor).
        if page_idx + 1 == ZULIP_MAX_PAGES {
            break;
        }
        anchor = next_anchor_id.to_string();
    }

    // Group messages for the target day by stream (or DM bucket).
    let mut groups: HashMap<String, Vec<ZulipMessage>> = HashMap::new();
    for msg in in_window {
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
                url: sanitize_event_url(&msg_url),
            },
        ));
    }

    rows.sort_by_key(|(ts, _)| *ts);
    Ok(rows)
}


#[derive(Deserialize)]
struct MessagesResponse {
    messages: Vec<ZulipMessage>,
    #[serde(default)]
    found_oldest: Option<bool>,
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

        let mut resp = ureq::get(&url)
            .header("Authorization", &auth)
            .header("Accept", "application/json")
            .call()
            .expect("Zulip API request failed");

        assert_eq!(resp.status().as_u16(), 200);
        let body: serde_json::Value = resp.body_mut().read_json().expect("Valid JSON from Zulip /users/me");
        assert_eq!(
            body.get("result").and_then(|v| v.as_str()),
            Some("success"),
            "Zulip /users/me result was not 'success': {body}"
        );
    }
}
