use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use serde::Deserialize;
use serde_json::Value;
use tauri::State;

use crate::commands::settings_github::{get_settings_github, GitHubEvent};
use crate::state::AppState;
use crate::timeline::{sanitize_event_url, TimelineEvent, TimelineEventSource};

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

    let next_end = end_day
        .succ_opt()
        .ok_or_else(|| format!("no day after {end_day}"))?;
    let since = start_day.and_hms_opt(0, 0, 0).ok_or("Invalid range start")?;
    let until = next_end.and_hms_opt(0, 0, 0).ok_or("Invalid range end")?;

    let since_local = Local
        .from_local_datetime(&since)
        .single()
        .ok_or("Ambiguous local start of range")?;
    let until_local = Local
        .from_local_datetime(&until)
        .single()
        .ok_or("Ambiguous local end of range")?;

    let raw_events =
        rest_api_user_events(&settings.username, &settings.token, since_local.timestamp())?;

    let mut rows: Vec<(NaiveDate, i64, TimelineEvent)> = Vec::new();

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
        // until_local is the exclusive upper bound (midnight after end_day).
        if local < since_local || local >= until_local {
            continue;
        }
        let day = local.naive_local().date();

        let ts = local.timestamp();
        let time = local.format("%H:%M").to_string();
        let id = format!("github:{}", github_event_id_str(&ev.id));

        rows.push((
            day,
            ts,
            TimelineEvent {
                id,
                time,
                source: TimelineEventSource::Github,
                title: mapped.title,
                detail: Some(mapped.detail),
                url: mapped.url.and_then(|u| sanitize_event_url(&u)),
            },
        ));
    }

    rows.sort_by_key(|(_, ts, _)| *ts);
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
    // The `/users/{username}/events` endpoint strips `html_url` from nested
    // payload objects (only api.github.com URLs remain), so every html_url is
    // reconstructed from the repo slug plus the event's number/id fields.
    match ev.event_type.as_str() {
        "PullRequestEvent" => {
            let action = j_str(&ev.payload, &["action"])?;
            let pr = ev.payload.get("pull_request")?;
            let number = j_u64(pr, &["number"]).unwrap_or(0);
            let url = Some(format!("https://github.com/{repo}/pull/{number}"));
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
            let url = match j_u64(review, &["id"]) {
                Some(review_id) => Some(format!(
                    "https://github.com/{repo}/pull/{number}#pullrequestreview-{review_id}"
                )),
                None => Some(format!("https://github.com/{repo}/pull/{number}")),
            };
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
            let url = match j_u64(comment, &["id"]) {
                Some(comment_id) => Some(format!(
                    "https://github.com/{repo}/pull/{number}#discussion_r{comment_id}"
                )),
                None => Some(format!("https://github.com/{repo}/pull/{number}")),
            };
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
            let url = Some(format!("https://github.com/{repo}/issues/{number}"));
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
            // PR comments arrive as IssueCommentEvents too — use `pull/` when
            // the issue has a pull_request field so the anchor lands on the
            // PR's conversation tab, not the issue page.
            let is_pr = issue.get("pull_request").is_some();
            let path = if is_pr { "pull" } else { "issues" };
            let url = match j_u64(comment, &["id"]) {
                Some(comment_id) => Some(format!(
                    "https://github.com/{repo}/{path}/{number}#issuecomment-{comment_id}"
                )),
                None => Some(format!("https://github.com/{repo}/{path}/{number}")),
            };
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

fn rest_api_user_events(
    username: &str,
    token: &str,
    since_ts: i64,
) -> Result<Vec<GhEvent>, String> {
    let auth = format!("Bearer {token}");
    let mut all_events: Vec<GhEvent> = Vec::new();

    for page in 1u32..=10 {
        let url = format!(
            "https://api.github.com/users/{}/events?per_page=100&page={}",
            urlencoding::encode(username),
            page
        );
        let mut response = match ureq::get(&url)
            .header("Authorization", &auth)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "recall-app")
            .call()
        {
            Ok(r) => r,
            Err(ureq::Error::StatusCode(status)) => {
                // Silently stop paginating once GitHub says "no more pages"
                // (422 is returned past the events-API page limit); surface
                // real failures (auth, server errors) so the UI can show them.
                if status == 422 && page > 1 {
                    break;
                }
                return Err(format!(
                    "GitHub API returned HTTP {status} (check your username and token)"
                ));
            }
            Err(e) => return Err(format!("GitHub API request failed: {e}")),
        };
        let page_events: Vec<GhEvent> = response
            .body_mut()
            .read_json()
            .map_err(|e| format!("Could not parse GitHub events JSON: {e}"))?;

        if page_events.is_empty() {
            break;
        }

        // Events are returned newest-first. If the last event on this page is
        // already older than the target day's start, every subsequent page
        // will be older too — stop paginating to save API calls.
        let oldest_ts = page_events
            .last()
            .and_then(|e| parse_github_datetime(&e.created_at).ok())
            .map(|dt| dt.timestamp());
        all_events.extend(page_events);
        if let Some(ts) = oldest_ts {
            if ts < since_ts {
                break;
            }
        }
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
        let result = rest_api_user_events(&username, &token, 0);
        assert!(
            result.is_ok(),
            "GitHub API call failed: {:?}",
            result.err()
        );
    }
}
