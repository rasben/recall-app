use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Local, NaiveDate, Utc};
use serde::Deserialize;
use tauri::State;
use ureq::Response;

use crate::commands::settings_jira::get_settings_jira;
use crate::state::AppState;
use crate::timeline::{TimelineEvent, TimelineEventSource};

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

fn jira_request_json(
    label: &str,
    method: &str,
    url: &str,
    email: &str,
    api_token: &str,
    json_body: Option<&str>,
) -> Result<(u16, String), String> {
    let auth = jira_basic_auth(email, api_token);
    let resp: Result<Response, ureq::Error> = match (method, json_body) {
        ("GET", None) => ureq::get(url)
            .set("Authorization", &auth)
            .set("Accept", "application/json")
            .call(),
        ("POST", Some(body)) => ureq::post(url)
            .set("Authorization", &auth)
            .set("Accept", "application/json")
            .set("Content-Type", "application/json")
            .send_string(body),
        ("GET", Some(_)) => {
            return Err("jira_request_json: GET with body is not supported".into());
        }
        ("POST", None) => {
            return Err("jira_request_json: POST requires a JSON body".into());
        }
        _ => return Err(format!("unsupported HTTP method {method}")),
    };

    match resp {
        Ok(r) => {
            let status = r.status();
            let body = r
                .into_string()
                .map_err(|e| format!("read body ({label}): {e}"))?;
            Ok((status, body))
        }
        Err(ureq::Error::Status(status, r)) => {
            let body = r.into_string().unwrap_or_default();
            Ok((status, body))
        }
        Err(e) => Err(format!("Jira HTTP ({label}): {e}")),
    }
}

pub(super) fn events_for_day(
    state: &State<'_, AppState>,
    day: &str,
) -> Result<Vec<(i64, TimelineEvent)>, String> {
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
        return Ok(Vec::new());
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

    let day_naive =
        NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|_| format!("Invalid date: {day}"))?;
    let next = day_naive
        .succ_opt()
        .ok_or_else(|| format!("no day after {day}"))?;
    let day_from = day_naive.format("%Y-%m-%d").to_string();
    let day_to = next.format("%Y-%m-%d").to_string();
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
        return Ok(Vec::new());
    }

    let parsed: SearchResponse =
        serde_json::from_str(&search_body).map_err(|e| format!("Jira search JSON: {e}"))?;

    let mut rows: Vec<(i64, TimelineEvent)> = Vec::new();
    for issue in parsed.issues {
        let updated = match parse_jira_updated(&issue.fields.updated) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let local = updated.with_timezone(&Local);
        if local.date_naive() != day_naive {
            continue;
        }

        let action = jira_action_label(account_id, day_naive, &issue.fields);
        let ts = local.timestamp();
        let time = local.format("%H:%M").to_string();
        let browse = format!("{base}/browse/{}", issue.key);
        let title = format!("{} {}", action, issue.key);
        let detail = Some(issue.fields.summary.clone());

        rows.push((
            ts,
            TimelineEvent {
                id: format!("jira:{}:{}:{}", issue.key, day_naive, action),
                time,
                source: TimelineEventSource::Jira,
                title,
                detail,
                url: Some(browse),
            },
        ));
    }

    rows.sort_by_key(|(ts, _)| *ts);
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
