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

pub(super) fn test_connection(state: &State<'_, AppState>) -> Result<(), String> {
    let Some(settings) = get_settings_zulip(state.clone()) else {
        return Err("Zulip is not configured".into());
    };
    let realm = normalize_realm_url(&settings.realm_url);
    if realm.is_empty() {
        return Err("Realm URL is required".into());
    }
    if settings.email.trim().is_empty() || settings.api_key.trim().is_empty() {
        return Err("Email and API key are required".into());
    }
    let url = format!("{realm}/api/v1/users/me");
    let auth = format!(
        "Basic {}",
        STANDARD.encode(format!("{}:{}", settings.email.trim(), settings.api_key.trim()))
    );
    match ureq::get(&url)
        .header("Authorization", &auth)
        .header("Accept", "application/json")
        .call()
    {
        Ok(_) => Ok(()),
        Err(ureq::Error::StatusCode(status)) => Err(format!(
            "Zulip returned HTTP {status} — check realm URL, email, and API key"
        )),
        Err(e) => Err(format!("Zulip request failed: {e}")),
    }
}

pub(super) fn events_for_day(
    state: &State<'_, AppState>,
    day: &str,
) -> Result<Vec<(i64, TimelineEvent)>, String> {
    let day_naive =
        NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|_| format!("Invalid date: {day}"))?;
    let rows = events_for_range(state, day_naive, day_naive)?;
    Ok(rows.into_iter().map(|(_, ts, ev)| (ts, ev)).collect())
}

pub(super) fn events_for_range(
    state: &State<'_, AppState>,
    start_day: NaiveDate,
    end_day: NaiveDate,
) -> Result<Vec<(NaiveDate, i64, TimelineEvent)>, String> {
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

    let next_end = end_day
        .succ_opt()
        .ok_or_else(|| format!("no day after {end_day}"))?;

    let range_start = Local
        .from_local_datetime(&start_day.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .map(|d| d.timestamp())
        .unwrap_or(0);
    let range_end = Local
        .from_local_datetime(&next_end.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .map(|d| d.timestamp())
        .unwrap_or(i64::MAX);

    let narrow = serde_json::json!([{"operator": "sender", "operand": email}]);
    let narrow_str =
        serde_json::to_string(&narrow).map_err(|e| format!("Zulip narrow JSON: {e}"))?;

    let auth = format!("Basic {}", STANDARD.encode(format!("{email}:{api_key}")));

    // The /api/v1/messages endpoint has no date filter, so we page backwards
    // from "newest" until we either (a) see a message older than range_start
    // (meaning the range is fully covered) or (b) the server reports there
    // are no older messages.
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
            if msg.timestamp >= range_start && msg.timestamp < range_end {
                in_window.push(msg);
            }
        }

        // Reached the far side of the target range, or Zulip says there's
        // nothing older than what we just got.
        if earliest_ts < range_start || parsed.found_oldest.unwrap_or(false) {
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

    // Bucket per (local day, stream-or-DM) and aggregate each bucket into a
    // single "Sent N messages…" event — same grouping semantics as the
    // per-day fetch, just applied across every day in the range at once.
    let mut groups: HashMap<(NaiveDate, String), Vec<ZulipMessage>> = HashMap::new();
    for msg in in_window {
        let local_day = DateTime::<Utc>::from_timestamp(msg.timestamp, 0)
            .map(|d| d.with_timezone(&Local).date_naive());
        let Some(local_day) = local_day else {
            continue;
        };
        let key = if msg.msg_type == "stream" {
            msg.display_recipient
                .as_str()
                .unwrap_or("unknown")
                .to_string()
        } else {
            format!("__dm__:{}", dm_conversation_key(&msg, &email))
        };
        groups.entry((local_day, key)).or_default().push(msg);
    }

    let mut rows: Vec<(NaiveDate, i64, TimelineEvent)> = Vec::new();
    for ((day_naive, stream_key), mut msgs) in groups {
        msgs.sort_by_key(|m| m.timestamp);
        let earliest_ts = msgs[0].timestamp;
        let last_ts = msgs[msgs.len() - 1].timestamp;
        let count = msgs.len();
        let is_dm = stream_key.starts_with("__dm__");

        let to_local_hhmm = |ts: i64| -> Result<String, String> {
            Ok(DateTime::<Utc>::from_timestamp(ts, 0)
                .ok_or_else(|| format!("invalid timestamp {ts}"))?
                .with_timezone(&Local)
                .format("%H:%M")
                .to_string())
        };
        let time = to_local_hhmm(earliest_ts)?;

        // Presence window across the bucket: when the user was active in this
        // conversation, NOT time spent — a whole-day bucket can stretch hours
        // around a handful of messages. Appended at the end so the count and
        // stream identity survive the one-line clamp.
        let span = if count > 1 && last_ts != earliest_ts {
            format!(" ({time}–{})", to_local_hhmm(last_ts)?)
        } else {
            String::new()
        };

        let noun = if count == 1 { "message" } else { "messages" };

        // For DMs, the conversation partners (deduplicated, excluding the sender).
        let recipient_names = if is_dm {
            let mut seen_names: Vec<String> = Vec::new();
            for m in &msgs {
                let Some(recipients) = m.display_recipient.as_array() else {
                    continue;
                };
                for r in recipients {
                    let Some(name) = r.get("full_name").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    let recipient_email = r.get("email").and_then(|v| v.as_str()).unwrap_or("");
                    if recipient_email.eq_ignore_ascii_case(&email) {
                        continue;
                    }
                    let name_s = name.to_string();
                    if !seen_names.contains(&name_s) {
                        seen_names.push(name_s);
                    }
                }
            }
            seen_names.join(", ")
        } else {
            String::new()
        };

        // For streams, the most-discussed topic (earliest message wins ties),
        // surfaced only when the bucket actually spans more than one real topic.
        let dominant_topic = if is_dm {
            None
        } else {
            let mut counts: Vec<(&str, usize)> = Vec::new();
            for m in &msgs {
                let t = m.subject.as_deref().unwrap_or("(no topic)");
                match counts.iter_mut().find(|(name, _)| *name == t) {
                    Some(entry) => entry.1 += 1,
                    None => counts.push((t, 1)),
                }
            }
            if counts.len() > 1 {
                let mut best: Option<(&str, usize)> = None;
                for &(name, c) in &counts {
                    if best.is_none_or(|(_, bc)| c > bc) {
                        best = Some((name, c));
                    }
                }
                best.map(|(name, _)| name.to_string())
                    .filter(|t| t != "(no topic)")
            } else {
                None
            }
        };

        let mut title = if is_dm {
            if recipient_names.is_empty() {
                format!("Sent {count} direct {noun}")
            } else {
                format!("Sent {count} direct {noun} to {recipient_names}")
            }
        } else {
            format!("Sent {count} {noun} in #{stream_key}")
        };
        if let Some(topic) = &dominant_topic {
            title.push_str(&format!(" · {topic}"));
        }
        title.push_str(&span);

        // detail: streams keep the full deduplicated topic list; DMs now carry
        // their partners in the title, so they need no detail line.
        let detail = if is_dm {
            None
        } else {
            let mut seen_topics: Vec<&str> = Vec::new();
            for m in &msgs {
                let t = m.subject.as_deref().unwrap_or("(no topic)");
                if !seen_topics.contains(&t) {
                    seen_topics.push(t);
                }
            }
            Some(seen_topics.join(", "))
        };

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

        let day_iso = day_naive.format("%Y-%m-%d").to_string();
        let id = if is_dm {
            let conversation_key = stream_key.strip_prefix("__dm__:").unwrap_or("self");
            format!("zulip:dm:{day_iso}:{conversation_key}")
        } else {
            format!("zulip:stream:{}:{day_iso}", stream_key)
        };

        rows.push((
            day_naive,
            earliest_ts,
            TimelineEvent {
                id,
                time,
                timestamp: earliest_ts,
                source: TimelineEventSource::Zulip,
                title,
                detail,
                url: sanitize_event_url(&msg_url),
            },
        ));
    }

    rows.sort_by_key(|(_, ts, _)| *ts);
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

/// Stable conversation key for a DM: the other participants' emails, lowercased,
/// deduplicated, and sorted so the same thread always hashes to the same bucket.
/// The sender is excluded; a note-to-self (no other participant) becomes "self".
fn dm_conversation_key(msg: &ZulipMessage, sender_email: &str) -> String {
    let mut emails: Vec<String> = Vec::new();
    if let Some(recipients) = msg.display_recipient.as_array() {
        for r in recipients {
            let Some(recipient_email) = r.get("email").and_then(|v| v.as_str()) else {
                continue;
            };
            if recipient_email.eq_ignore_ascii_case(sender_email) {
                continue;
            }
            let lower = recipient_email.to_ascii_lowercase();
            if !emails.contains(&lower) {
                emails.push(lower);
            }
        }
    }
    if emails.is_empty() {
        return "self".to_string();
    }
    emails.sort();
    emails.join(",")
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

    fn dm_msg(recipients: serde_json::Value) -> ZulipMessage {
        ZulipMessage {
            id: 1,
            timestamp: 0,
            msg_type: "private".to_string(),
            subject: None,
            display_recipient: recipients,
            stream_id: None,
        }
    }

    #[test]
    fn dm_conversation_key_excludes_sender_and_sorts() {
        let msg = dm_msg(serde_json::json!([
            {"email": "me@x.com", "full_name": "Me"},
            {"email": "Bob@x.com", "full_name": "Bob"},
            {"email": "alice@x.com", "full_name": "Alice"}
        ]));
        // Sender dropped (case-insensitive), rest lowercased and sorted.
        assert_eq!(dm_conversation_key(&msg, "ME@x.com"), "alice@x.com,bob@x.com");
    }

    #[test]
    fn dm_conversation_key_note_to_self() {
        let msg = dm_msg(serde_json::json!([{"email": "me@x.com", "full_name": "Me"}]));
        assert_eq!(dm_conversation_key(&msg, "me@x.com"), "self");
    }

    #[test]
    fn dm_conversation_key_deduplicates() {
        let msg = dm_msg(serde_json::json!([
            {"email": "alice@x.com", "full_name": "Alice"},
            {"email": "alice@x.com", "full_name": "Alice"}
        ]));
        assert_eq!(dm_conversation_key(&msg, "me@x.com"), "alice@x.com");
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
