// To-Do - this file is completely AI-coded, and not well-reviewed.
// We need to review it, and optimize it.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Local, NaiveDate, Utc};
use serde::Deserialize;
use tauri::State;
use crate::commands::settings_jira::get_settings_jira;
use crate::state::AppState;
use crate::timeline::{sanitize_event_url, TimelineEvent, TimelineEventSource};

/// Quote a user id / account id for use inside JQL `updatedBy("…")`.
fn jql_quoted_identifier(id: &str) -> String {
    let escaped = id.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn normalize_site_url(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_string()
}

fn jira_basic_auth(email: &str, api_token: &str) -> String {
    let raw = format!("{email}:{api_token}");
    format!("Basic {}", STANDARD.encode(raw.as_bytes()))
}

pub(crate) fn jira_request_json(
    label: &str,
    method: &str,
    url: &str,
    email: &str,
    api_token: &str,
    json_body: Option<&str>,
) -> Result<(u16, String), String> {
    let auth = jira_basic_auth(email, api_token);
    let resp = match (method, json_body) {
        ("GET", None) => ureq::get(url)
            .header("Authorization", &auth)
            .header("Accept", "application/json")
            .call(),
        ("POST", Some(body)) => ureq::post(url)
            .header("Authorization", &auth)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .send(body),
        ("GET", Some(_)) => {
            return Err("jira_request_json: GET with body is not supported".into());
        }
        ("POST", None) => {
            return Err("jira_request_json: POST requires a JSON body".into());
        }
        _ => return Err(format!("unsupported HTTP method {method}")),
    };

    let mut r = match resp {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(status)) => return Ok((status, String::new())),
        Err(e) => return Err(format!("Jira HTTP ({label}): {e}")),
    };

    let status = r.status().as_u16();
    let body = r.body_mut().read_to_string().map_err(|e| format!("read body ({label}): {e}"))?;
    Ok((status, body))
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
    let Some(settings) = get_settings_jira(state.clone()) else {
        return Ok(Vec::new());
    };
    if !settings.enabled {
        return Ok(Vec::new());
    }
    let base = normalize_site_url(&settings.site_url);
    if base.is_empty() {
        return Ok(Vec::new());
    }
    if settings.email.trim().is_empty() || settings.api_token.trim().is_empty() {
        return Ok(Vec::new());
    }

    let email = settings.email.trim();
    let token = settings.api_token.trim();

    let myself_url = format!("{base}/rest/api/3/myself");
    let (myself_status, myself_body) =
        jira_request_json("myself", "GET", &myself_url, email, token, None)?;
    if myself_status >= 400 {
        return Err(format!(
            "Jira /myself returned HTTP {myself_status} (check site URL, email, and API token)"
        ));
    }
    let myself: serde_json::Value =
        serde_json::from_str(&myself_body).map_err(|e| format!("Jira myself JSON: {e}"))?;
    let Some(account_id) = myself
        .get("accountId")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    else {
        return Ok(Vec::new());
    };

    let next_end = end_day
        .succ_opt()
        .ok_or_else(|| format!("no day after {end_day}"))?;
    let day_from = start_day.format("%Y-%m-%d").to_string();
    let day_to = next_end.format("%Y-%m-%d").to_string();
    // Only issues you touched: created, field/workflow edits, comments (per Atlassian `updatedBy()`).
    // `currentUser()` is not supported inside `updatedBy()`, so we use `accountId` from /myself.
    // https://support.atlassian.com/jira-software-cloud/docs/advanced-search-reference-jql-functions/
    let user_lit = jql_quoted_identifier(account_id);
    let jql = format!(
        r#"workItemKey IN updatedBy({user_lit}, "{day_from}", "{day_to}") ORDER BY updated DESC"#
    );

    // Legacy GET /rest/api/3/search returns 410; use enhanced search (CHANGE-2046).
    let search_url = format!("{base}/rest/api/3/search/jql");
    let search_payload = serde_json::json!({
        "jql": jql,
        "maxResults": 100,
        "fields": ["summary", "updated", "created", "creator", "comment"]
    });
    let search_body_json = serde_json::to_string(&search_payload)
        .map_err(|e| format!("Jira search request JSON: {e}"))?;

    let (search_status, search_body) = jira_request_json(
        "search",
        "POST",
        &search_url,
        email,
        token,
        Some(&search_body_json),
    )?;
    if search_status >= 400 {
        return Err(format!(
            "Jira search returned HTTP {search_status}: {}",
            search_body.chars().take(200).collect::<String>()
        ));
    }

    let parsed: SearchResponse =
        serde_json::from_str(&search_body).map_err(|e| format!("Jira search JSON: {e}"))?;

    let mut rows: Vec<(NaiveDate, i64, TimelineEvent)> = Vec::new();
    for issue in parsed.issues {
        let updated = match parse_jira_updated(&issue.fields.updated) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let local = updated.with_timezone(&Local);
        let day_naive = local.date_naive();
        if day_naive < start_day || day_naive > end_day {
            continue;
        }

        let action = jira_action_label(account_id, day_naive, &issue.fields);
        let ts = local.timestamp();
        let time = local.format("%H:%M").to_string();
        let browse = format!("{base}/browse/{}", issue.key);
        let title = format!("{} {}", action, issue.key);
        let detail = Some(issue.fields.summary.clone());

        rows.push((
            day_naive,
            ts,
            TimelineEvent {
                id: format!("jira:{}:{}:{}", issue.key, day_naive, action),
                time,
                source: TimelineEventSource::Jira,
                title,
                detail,
                url: sanitize_event_url(&browse),
            },
        ));
    }

    rows.sort_by_key(|(_, ts, _)| *ts);
    Ok(rows)
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    issues: Vec<JiraIssue>,
}

#[derive(Debug, Deserialize)]
struct JiraIssue {
    key: String,
    fields: JiraFields,
}

#[derive(Debug, Deserialize)]
struct JiraFields {
    summary: String,
    updated: String,
    #[serde(default)]
    created: Option<String>,
    #[serde(default)]
    creator: Option<JiraAccountUser>,
    #[serde(default)]
    comment: Option<JiraCommentBlock>,
}

#[derive(Debug, Deserialize)]
struct JiraAccountUser {
    #[serde(rename = "accountId")]
    account_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct JiraCommentBlock {
    comments: Vec<JiraComment>,
}

#[derive(Debug, Deserialize)]
struct JiraComment {
    #[serde(default)]
    author: Option<JiraAccountUser>,
    #[serde(default)]
    created: Option<String>,
}

/// Prefer the most specific label for *your* activity on this calendar day (search may omit some comments).
fn jira_action_label(
    my_account_id: &str,
    day_naive: NaiveDate,
    fields: &JiraFields,
) -> &'static str {
    if let Some(block) = &fields.comment {
        for c in &block.comments {
            if c.author.as_ref().and_then(|a| a.account_id.as_deref()) != Some(my_account_id) {
                continue;
            }
            let Some(ref created) = c.created else {
                continue;
            };
            let Ok(dt) = parse_jira_updated(created) else {
                continue;
            };
            if dt.with_timezone(&Local).date_naive() == day_naive {
                return "Commented";
            }
        }
    }
    if let (Some(creator), Some(ref created)) = (&fields.creator, &fields.created) {
        if creator.account_id.as_deref() == Some(my_account_id) {
            if let Ok(dt) = parse_jira_updated(created) {
                if dt.with_timezone(&Local).date_naive() == day_naive {
                    return "Created";
                }
            }
        }
    }
    "Edited"
}

/// Jira often returns offsets like `+0100`; RFC3339 expects `+01:00`.
fn jira_timestamp_to_rfc3339(s: &str) -> String {
    if s.len() < 5 {
        return s.to_string();
    }
    let (main, tail) = s.split_at(s.len() - 5);
    let b = tail.as_bytes();
    if (b[0] != b'+' && b[0] != b'-') || !tail[1..].chars().all(|c| c.is_ascii_digit()) {
        return s.to_string();
    }
    format!("{}{}:{}", main, &tail[0..3], &tail[3..5])
}

fn parse_jira_updated(s: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(s)
        .or_else(|_| DateTime::parse_from_rfc3339(&jira_timestamp_to_rfc3339(s)))
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| format!("unrecognized Jira datetime {s:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    // --- pure helpers ---

    #[test]
    fn jira_timestamp_to_rfc3339_converts_offset() {
        assert_eq!(
            jira_timestamp_to_rfc3339("2024-01-15T10:30:00.000+0100"),
            "2024-01-15T10:30:00.000+01:00"
        );
        assert_eq!(
            jira_timestamp_to_rfc3339("2024-01-15T10:30:00.000-0530"),
            "2024-01-15T10:30:00.000-05:30"
        );
    }

    #[test]
    fn jira_timestamp_to_rfc3339_leaves_rfc3339_alone() {
        let s = "2024-01-15T10:30:00+01:00";
        assert_eq!(jira_timestamp_to_rfc3339(s), s);
    }

    #[test]
    fn parse_jira_updated_valid_rfc3339() {
        assert!(parse_jira_updated("2024-01-15T10:30:00+00:00").is_ok());
    }

    #[test]
    fn parse_jira_updated_jira_offset_format() {
        assert!(parse_jira_updated("2024-01-15T10:30:00.000+0100").is_ok());
    }

    #[test]
    fn parse_jira_updated_invalid() {
        assert!(parse_jira_updated("not-a-date").is_err());
    }

    #[test]
    fn jql_quoted_identifier_plain() {
        assert_eq!(jql_quoted_identifier("abc123"), "\"abc123\"");
    }

    #[test]
    fn jql_quoted_identifier_escapes_quotes() {
        assert_eq!(jql_quoted_identifier("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn jql_quoted_identifier_escapes_backslash() {
        assert_eq!(jql_quoted_identifier("a\\b"), "\"a\\\\b\"");
    }

    #[test]
    fn normalize_site_url_strips_trailing_slash() {
        assert_eq!(
            normalize_site_url("https://example.atlassian.net/"),
            "https://example.atlassian.net"
        );
    }

    #[test]
    fn normalize_site_url_unchanged_when_clean() {
        assert_eq!(
            normalize_site_url("https://example.atlassian.net"),
            "https://example.atlassian.net"
        );
    }

    #[test]
    fn jira_basic_auth_correct_base64() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let result = jira_basic_auth("user@example.com", "mytoken");
        let expected = format!("Basic {}", STANDARD.encode("user@example.com:mytoken"));
        assert_eq!(result, expected);
    }

    #[test]
    fn jira_action_label_commented() {
        let day = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let fields = JiraFields {
            summary: "Test issue".to_string(),
            updated: "2024-01-15T10:00:00+00:00".to_string(),
            created: None,
            creator: None,
            comment: Some(JiraCommentBlock {
                comments: vec![JiraComment {
                    author: Some(JiraAccountUser {
                        account_id: Some("user123".to_string()),
                    }),
                    created: Some("2024-01-15T10:00:00+00:00".to_string()),
                }],
            }),
        };
        assert_eq!(jira_action_label("user123", day, &fields), "Commented");
    }

    #[test]
    fn jira_action_label_created() {
        let day = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let fields = JiraFields {
            summary: "New issue".to_string(),
            updated: "2024-01-15T09:00:00+00:00".to_string(),
            created: Some("2024-01-15T09:00:00+00:00".to_string()),
            creator: Some(JiraAccountUser {
                account_id: Some("user123".to_string()),
            }),
            comment: None,
        };
        assert_eq!(jira_action_label("user123", day, &fields), "Created");
    }

    #[test]
    fn jira_action_label_default_edited() {
        let day = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let fields = JiraFields {
            summary: "Some issue".to_string(),
            updated: "2024-01-15T10:00:00+00:00".to_string(),
            created: None,
            creator: None,
            comment: None,
        };
        assert_eq!(jira_action_label("user123", day, &fields), "Edited");
    }

    // --- integration (skipped when secrets absent) ---

    #[test]
    fn jira_myself_returns_account_id_with_valid_credentials() {
        let site_url = match std::env::var("RECALL_TEST_JIRA_SITE_URL") {
            Ok(u) => u,
            Err(_) => return,
        };
        let email = match std::env::var("RECALL_TEST_JIRA_EMAIL") {
            Ok(e) => e,
            Err(_) => return,
        };
        let token = match std::env::var("RECALL_TEST_JIRA_API_TOKEN") {
            Ok(t) => t,
            Err(_) => return,
        };

        let base = normalize_site_url(&site_url);
        let url = format!("{base}/rest/api/3/myself");
        let result = jira_request_json("myself", "GET", &url, &email, &token, None);
        assert!(result.is_ok(), "Jira request failed: {:?}", result.err());

        let (status, body) = result.unwrap();
        assert_eq!(status, 200, "Jira /myself returned non-200: {body}");

        let v: serde_json::Value =
            serde_json::from_str(&body).expect("Invalid JSON from Jira /myself");
        assert!(
            v.get("accountId").and_then(|id| id.as_str()).is_some(),
            "No accountId in Jira /myself response: {body}"
        );
    }
}
