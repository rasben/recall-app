use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use serde::Deserialize;
use serde_json::Value;
use tauri::State;

use crate::commands::settings_github::{get_settings_github, GitHubEvent};
use crate::state::AppState;
use crate::timeline::{TimelineEvent, TimelineEventSource};

#[derive(Debug, Deserialize)]
struct GhEvent {
    id: Value,
    #[serde(rename = "type")]
    event_type: String,
    repo: GhRepo,
    #[serde(default)]
    payload: Value,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct GhRepo {
    name: String,
}

pub(super) fn events_for_day(
    state: &State<'_, AppState>,
    day: &str,
) -> Result<Vec<(i64, TimelineEvent)>, String> {
    let Some(settings) = get_settings_github(state.clone()) else {
        return Ok(Vec::new());
    };
    if !settings.enabled {
        return Ok(Vec::new());
    }
    if settings.username.is_empty() || settings.token.is_empty() {
        return Ok(Vec::new());
    }
    if settings.enabled_events.is_empty() {
        return Ok(Vec::new());
    }

    let day_naive =
        NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|_| format!("Invalid date: {day}"))?;

    let since = day_naive.and_hms_opt(0, 0, 0).ok_or("Invalid day start")?;
    let until = day_naive.and_hms_opt(23, 59, 59).ok_or("Invalid day end")?;

    let since_local = Local
        .from_local_datetime(&since)
        .single()
        .ok_or("Ambiguous local start of day")?;
    let until_local = Local
        .from_local_datetime(&until)
        .single()
        .ok_or("Ambiguous local end of day")?;

    let raw_events = rest_api_user_events(&settings.username, &settings.token)?;

    let mut rows: Vec<(i64, TimelineEvent)> = Vec::new();

    for ev in raw_events {
        if matches!(ev.event_type.as_str(), "PushEvent") {
            continue;
        }

        let Some(kind) = github_api_event_type(&ev.event_type) else {
            continue;
        };
        if !settings.enabled_events.contains(&kind) {
            continue;
        }

        let Some(mapped) = map_github_event(&ev) else {
            continue;
        };

        let created = parse_github_datetime(&ev.created_at)?;
        let local = created.with_timezone(&Local);
        if local.naive_local().date() != day_naive {
            continue;
        }
        if local < since_local || local > until_local {
            continue;
        }

        let ts = local.timestamp();
        let time = local.format("%H:%M").to_string();
        let id = format!("github:{}", github_event_id_str(&ev.id));

        rows.push((
            ts,
            TimelineEvent {
                id,
                time,
                source: TimelineEventSource::Github,
                title: mapped.title,
                detail: Some(mapped.detail),
                url: mapped.url,
            },
        ));
    }

    rows.sort_by_key(|(ts, _)| *ts);
    Ok(rows)
}

struct MappedGithub {
    title: String,
    detail: String,
    url: Option<String>,
}

fn github_api_event_type(event_type: &str) -> Option<GitHubEvent> {
    match event_type {
        "PullRequestEvent" => Some(GitHubEvent::PullRequestEvent),
        "PullRequestReviewEvent" => Some(GitHubEvent::PullRequestReviewEvent),
        "PullRequestReviewCommentEvent" => Some(GitHubEvent::PullRequestReviewCommentEvent),
        "IssuesEvent" => Some(GitHubEvent::IssuesEvent),
        "IssueCommentEvent" => Some(GitHubEvent::IssueCommentEvent),
        _ => None,
    }
}

fn map_github_event(ev: &GhEvent) -> Option<MappedGithub> {
    let repo = ev.repo.name.clone();
    match ev.event_type.as_str() {
        "PullRequestEvent" => {
            let action = j_str(&ev.payload, &["action"])?;
            let pr = ev.payload.get("pull_request")?;
            let number = j_u64(pr, &["number"]).unwrap_or(0);
            let url = j_str(pr, &["html_url"]);
            let title = format_with_optional_subject(
                format!("Pull request #{number} {action}"),
                pull_request_subject(pr),
            );
            Some(MappedGithub {
                title,
                detail: repo,
                url,
            })
        }
        "PullRequestReviewEvent" => {
            let review = ev.payload.get("review")?;
            let state = j_str(review, &["state"]).unwrap_or_else(|| "reviewed".to_string());
            let pr = ev.payload.get("pull_request")?;
            let number = j_u64(pr, &["number"]).unwrap_or(0);
            let url = j_str(review, &["html_url"]).or_else(|| j_str(pr, &["html_url"]));
            let title = format_with_optional_subject(
                format!("PR review ({state}) on #{number}"),
                pull_request_subject(pr),
            );
            Some(MappedGithub {
                title,
                detail: repo,
                url,
            })
        }
        "PullRequestReviewCommentEvent" => {
            let pr = ev.payload.get("pull_request")?;
            let number = j_u64(pr, &["number"]).unwrap_or(0);
            let comment = ev.payload.get("comment")?;
            let url = j_str(comment, &["html_url"]);
            let title = format_with_optional_subject(
                format!("PR review comment on #{number}"),
                pull_request_subject(pr),
            );
            Some(MappedGithub {
                title,
                detail: repo,
                url,
            })
        }
        "IssuesEvent" => {
            let action = j_str(&ev.payload, &["action"])?;
            let issue = ev.payload.get("issue")?;
            let number = j_u64(issue, &["number"]).unwrap_or(0);
            let url = j_str(issue, &["html_url"]);
            let title = format_with_optional_subject(
                format!("Issue #{number} {action}"),
                issue_subject(issue),
            );
            Some(MappedGithub {
                title,
                detail: repo,
                url,
            })
        }
        "IssueCommentEvent" => {
            let issue = ev.payload.get("issue")?;
            let number = j_u64(issue, &["number"]).unwrap_or(0);
            let comment = ev.payload.get("comment")?;
            let url = j_str(comment, &["html_url"]);
            let title =
                format_with_optional_subject(format!("Comment on #{number}"), issue_subject(issue));
            Some(MappedGithub {
                title,
                detail: repo,
                url,
            })
        }
        _ => None,
    }
}

fn pull_request_subject(pr: &Value) -> Option<String> {
    j_nonempty_str(pr, &["title"])
        .or_else(|| j_nonempty_str(pr, &["head", "ref"]).map(|r| format!("branch {r}")))
}

fn issue_subject(issue: &Value) -> Option<String> {
    j_nonempty_str(issue, &["title"])
}

fn format_with_optional_subject(head: String, subject: Option<String>) -> String {
    match subject {
        Some(s) if !s.trim().is_empty() => format!("{head}: {s}"),
        _ => head,
    }
}

fn j_nonempty_str(v: &Value, path: &[&str]) -> Option<String> {
    j_str(v, path).filter(|s| !s.trim().is_empty())
}

fn github_event_id_str(id: &Value) -> String {
    match id {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => "unknown".into(),
    }
}

fn j_str(v: &Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn j_u64(v: &Value, path: &[&str]) -> Option<u64> {
    let mut cur = v;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_u64()
}

fn parse_github_datetime(s: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| s.parse::<DateTime<Utc>>())
        .map_err(|e| format!("Invalid GitHub timestamp {s:?}: {e}"))
}

pub(crate) fn rest_api_user_events(username: &str, token: &str) -> Result<Vec<GhEvent>, String> {
    let auth = format!("Bearer {token}");
    let mut all_events: Vec<GhEvent> = Vec::new();

    for page in 1u32..=10 {
        let url = format!(
            "https://api.github.com/users/{}/events?per_page=100&page={}",
            urlencoding::encode(username),
            page
        );
        let response = match ureq::get(&url)
            .set("Authorization", &auth)
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", "2022-11-28")
            .set("User-Agent", "recall-app")
            .call()
        {
            Ok(r) => r,
            Err(ureq::Error::Status(_, _)) => break,
            Err(e) => return Err(format!("GitHub API request failed: {e}")),
        };

        let page_events: Vec<GhEvent> = response
            .into_json()
            .map_err(|e| format!("Could not parse GitHub events JSON: {e}"))?;

        if page_events.is_empty() {
            break;
        }
        all_events.extend(page_events);
    }

    Ok(all_events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- pure helpers ---

    #[test]
    fn parse_github_datetime_valid() {
        assert!(parse_github_datetime("2024-01-15T10:30:00Z").is_ok());
        assert!(parse_github_datetime("2024-01-15T10:30:00+02:00").is_ok());
    }

    #[test]
    fn parse_github_datetime_invalid() {
        assert!(parse_github_datetime("not-a-date").is_err());
    }

    #[test]
    fn format_with_optional_subject_with_text() {
        assert_eq!(
            format_with_optional_subject("PR #42 opened".into(), Some("My feature".into())),
            "PR #42 opened: My feature"
        );
    }

    #[test]
    fn format_with_optional_subject_none() {
        assert_eq!(
            format_with_optional_subject("PR #42 opened".into(), None),
            "PR #42 opened"
        );
    }

    #[test]
    fn format_with_optional_subject_whitespace_only() {
        assert_eq!(
            format_with_optional_subject("PR #42 opened".into(), Some("   ".into())),
            "PR #42 opened"
        );
    }

    #[test]
    fn github_event_id_str_number() {
        assert_eq!(github_event_id_str(&json!(42)), "42");
    }

    #[test]
    fn github_event_id_str_string() {
        assert_eq!(github_event_id_str(&json!("abc123")), "abc123");
    }

    #[test]
    fn github_event_id_str_other() {
        assert_eq!(github_event_id_str(&json!(null)), "unknown");
    }

    #[test]
    fn j_str_found() {
        let v = json!({"a": {"b": "hello"}});
        assert_eq!(j_str(&v, &["a", "b"]), Some("hello".to_string()));
    }

    #[test]
    fn j_str_missing() {
        let v = json!({"a": {}});
        assert_eq!(j_str(&v, &["a", "b"]), None);
    }

    #[test]
    fn j_str_non_string() {
        let v = json!({"a": 42});
        assert_eq!(j_str(&v, &["a"]), None);
    }

    #[test]
    fn j_u64_found() {
        let v = json!({"number": 99});
        assert_eq!(j_u64(&v, &["number"]), Some(99u64));
    }

    #[test]
    fn j_u64_missing() {
        assert_eq!(j_u64(&json!({}), &["number"]), None);
    }

    #[test]
    fn github_api_event_type_known() {
        use crate::commands::settings_github::GitHubEvent;
        assert!(matches!(
            github_api_event_type("PullRequestEvent"),
            Some(GitHubEvent::PullRequestEvent)
        ));
        assert!(matches!(
            github_api_event_type("PullRequestReviewEvent"),
            Some(GitHubEvent::PullRequestReviewEvent)
        ));
        assert!(matches!(
            github_api_event_type("IssueCommentEvent"),
            Some(GitHubEvent::IssueCommentEvent)
        ));
    }

    #[test]
    fn github_api_event_type_unknown() {
        assert!(github_api_event_type("PushEvent").is_none());
        assert!(github_api_event_type("WatchEvent").is_none());
    }

    // --- integration (skipped when secrets absent) ---

    #[test]
    fn github_api_returns_events_with_valid_credentials() {
        let token = match std::env::var("RECALL_TEST_GITHUB_TOKEN") {
            Ok(t) => t,
            Err(_) => return,
        };
        let username = match std::env::var("RECALL_TEST_GITHUB_USERNAME") {
            Ok(u) => u,
            Err(_) => return,
        };
        let result = rest_api_user_events(&username, &token);
        assert!(
            result.is_ok(),
            "GitHub API call failed: {:?}",
            result.err()
        );
    }
}
