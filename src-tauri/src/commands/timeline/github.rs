/// To-Do - this file is completely AI-coded, and not well-reviewed.
/// We need to review it, and optimize it.
use std::process::Command;

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
    if !settings.enabled || !settings.use_cli {
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

    let login = gh_cli_login()?;
    let raw_events = gh_cli_user_events(&login)?;

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

/// Maps GitHub public timeline `type` strings to the settings enum (same names as the API).
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

/// User activity events often ship a slim `pull_request` without `title`; `head.ref` is usually present.
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

fn gh_cli_login() -> Result<String, String> {
    let output = Command::new("gh")
        .args(["api", "user", "--jq", ".login"])
        .output()
        .map_err(|e| format!("Failed to run `gh` (is GitHub CLI installed?): {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "`gh api user` failed. Run `gh auth login`. {}",
            stderr.trim()
        ));
    }

    let login = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if login.is_empty() {
        return Err("`gh api user` returned an empty login.".into());
    }
    Ok(login)
}

fn gh_cli_user_events(login: &str) -> Result<Vec<GhEvent>, String> {
    let endpoint = format!("users/{login}/events");
    let output = Command::new("gh")
        .arg("api")
        .arg("-H")
        .arg("Accept: application/vnd.github+json")
        .arg(&endpoint)
        .arg("--paginate")
        .output()
        .map_err(|e| format!("Failed to run `gh api {endpoint}`: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("`gh api {endpoint}` failed: {}", stderr.trim()));
    }

    serde_json::from_slice(&output.stdout).map_err(|e| {
        format!(
            "Could not parse GitHub events JSON from `gh`: {e}. Raw (truncated): {}",
            String::from_utf8_lossy(&output.stdout)
                .chars()
                .take(200)
                .collect::<String>()
        )
    })
}
