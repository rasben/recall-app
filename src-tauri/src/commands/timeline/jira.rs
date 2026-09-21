//! Jira Cloud timeline source.
//!
//! Endpoint behaviour verified against the Jira Cloud REST API v3 OpenAPI spec
//! (`https://developer.atlassian.com/cloud/jira/platform/swagger-v3.v3.json`):
//!
//! * `POST /rest/api/3/search/jql` (enhanced search) is cursor-paginated: the
//!   request carries `nextPageToken` (absent on the first page) and the response
//!   carries `nextPageToken` (null on the last page) plus `isLast`. There is no
//!   `total`. `maxResults` defaults to 50; Jira may return fewer per page when many
//!   fields are requested. The legacy `/rest/api/3/search` is deprecated
//!   ("currently being removed", CHANGE-2046) and returns 410 on most sites.
//!   `expand=changelog` adds each issue's most recent histories (newest first) with
//!   a `total`, which lets us skip the per-issue changelog call when it is complete.
//! * `GET /rest/api/3/issue/{key}/changelog` is offset-paginated
//!   (`startAt`/`maxResults`, default 100) and returns `values`, `total`, `isLast`,
//!   sorted oldest first. Each entry has `author.accountId`, `created` and `items`
//!   with `field`/`fieldId`.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Local, NaiveDate, Utc};
use serde::Deserialize;
use std::collections::BTreeMap;
use tauri::State;
use crate::commands::settings_jira::get_settings_jira;
use crate::state::AppState;
use crate::timeline::{sanitize_event_url, TimelineEvent, TimelineEventSource};

/// Issues requested per enhanced-search page. Jira may return fewer.
const SEARCH_PAGE_SIZE: u32 = 100;
/// Hard cap on enhanced-search pages walked for one range fetch
/// (`SEARCH_PAGE_SIZE × SEARCH_MAX_PAGES` = 5 000 issues). No realistic
/// month of one person's activity comes close; if the cap is hit we return
/// `Err` so `timeline/mod.rs` reports the gap instead of caching a partial range.
const SEARCH_MAX_PAGES: usize = 50;
/// Changelog entries requested per page of `GET /issue/{key}/changelog`.
const CHANGELOG_PAGE_SIZE: u32 = 100;
/// Hard cap on changelog pages walked for one issue
/// (`CHANGELOG_PAGE_SIZE × CHANGELOG_MAX_PAGES` = 2 000 history entries).
/// Exceeding it is treated as an error for the same partial-result reason.
const CHANGELOG_MAX_PAGES: usize = 20;

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

pub(super) fn test_connection(state: &State<'_, AppState>) -> Result<(), String> {
    let Some(settings) = get_settings_jira(state.clone()) else {
        return Err("Jira is not configured".into());
    };
    let base = normalize_site_url(&settings.site_url);
    if base.is_empty() {
        return Err("Site URL is required".into());
    }
    if settings.email.trim().is_empty() || settings.api_token.trim().is_empty() {
        return Err("Email and API token are required".into());
    }
    let url = format!("{base}/rest/api/3/myself");
    let (status, _) =
        jira_request_json("myself", "GET", &url, settings.email.trim(), settings.api_token.trim(), None)?;
    if status >= 400 {
        return Err(format!(
            "Jira returned HTTP {status} — check site URL, email, and API token"
        ));
    }
    Ok(())
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
    let issues = walk_search_pages(SEARCH_MAX_PAGES, |next_page_token| {
        let body = search_request_body(&jql, next_page_token)?;
        let (status, resp) =
            jira_request_json("search", "POST", &search_url, email, token, Some(&body))?;
        if status >= 400 {
            return Err(format!(
                "Jira search returned HTTP {status}: {}",
                resp.chars().take(200).collect::<String>()
            ));
        }
        Ok(resp)
    })?;

    let mut rows: Vec<(NaiveDate, i64, TimelineEvent)> = Vec::new();
    for issue in issues {
        // Prefer the histories that came inline with the search; only page through the
        // dedicated changelog endpoint when Jira truncated them.
        let inline = issue.changelog.unwrap_or_default();
        let histories = if inline_changelog_is_complete(&inline) {
            inline.histories
        } else {
            let changelog_url = format!("{base}/rest/api/3/issue/{}/changelog", issue.key);
            let mut denied = false;
            let walked = walk_changelog_pages(CHANGELOG_MAX_PAGES, |start_at| {
                let url = format!(
                    "{changelog_url}?startAt={start_at}&maxResults={CHANGELOG_PAGE_SIZE}"
                );
                let (status, resp) =
                    jira_request_json("changelog", "GET", &url, email, token, None)?;
                if status >= 400 {
                    // Per-issue permission/availability problem: degrade to the inline
                    // (possibly truncated) histories instead of failing the whole range.
                    denied = true;
                    return Ok(r#"{"values":[],"isLast":true}"#.to_string());
                }
                Ok(resp)
            })?;
            if denied { inline.histories } else { walked }
        };

        let actions = user_actions(account_id, start_day, end_day, &issue.fields, &histories);
        let browse = format!("{base}/browse/{}", issue.key);
        for a in actions {
            let local = DateTime::<Utc>::from_timestamp(a.first_ts, 0)
                .map(|dt| dt.with_timezone(&Local));
            let Some(local) = local else { continue };
            let label = a.action.label();
            let mut detail = issue.fields.summary.clone();
            if a.count > 1 {
                detail.push_str(&format!(" ×{}", a.count));
            }
            rows.push((
                a.day,
                a.first_ts,
                TimelineEvent {
                    id: format!("jira:{}:{}:{}", issue.key, a.day, label),
                    time: local.format("%H:%M").to_string(),
                    timestamp: a.first_ts,
                    source: TimelineEventSource::Jira,
                    title: format!("{} {}", label, issue.key),
                    detail: Some(detail),
                    url: sanitize_event_url(&browse),
                },
            ));
        }
    }

    rows.sort_by_key(|(_, ts, _)| *ts);
    Ok(rows)
}

/// JSON body for one enhanced-search page. `next_page_token` is omitted on the
/// first page (Jira rejects an explicit null on some sites) and echoed back
/// verbatim afterwards.
fn search_request_body(jql: &str, next_page_token: Option<&str>) -> Result<String, String> {
    let mut payload = serde_json::json!({
        "jql": jql,
        "maxResults": SEARCH_PAGE_SIZE,
        "fields": ["summary", "updated", "created", "creator", "comment"],
        "expand": "changelog",
    });
    if let Some(tok) = next_page_token {
        payload["nextPageToken"] = serde_json::Value::String(tok.to_string());
    }
    serde_json::to_string(&payload).map_err(|e| format!("Jira search request JSON: {e}"))
}

/// Walk enhanced-search pages until Jira reports the last one. `fetch_page`
/// receives the `nextPageToken` to send (`None` for the first page) and returns
/// the raw JSON body. Stops on `isLast: true`, a null/absent/empty
/// `nextPageToken`, or an empty page. Returns `Err` when `max_pages` is exhausted
/// or the server repeats a token, so callers never treat a truncated walk as
/// complete.
fn walk_search_pages<F>(max_pages: usize, mut fetch_page: F) -> Result<Vec<JiraIssue>, String>
where
    F: FnMut(Option<&str>) -> Result<String, String>,
{
    let mut issues: Vec<JiraIssue> = Vec::new();
    let mut token: Option<String> = None;
    for page in 1..=max_pages {
        let body = fetch_page(token.as_deref())?;
        let parsed: SearchResponse = serde_json::from_str(&body)
            .map_err(|e| format!("Jira search JSON (page {page}): {e}"))?;
        let got = parsed.issues.len();
        issues.extend(parsed.issues);
        let next = parsed.next_page_token.filter(|t| !t.is_empty());
        if parsed.is_last.unwrap_or(false) || next.is_none() || got == 0 {
            return Ok(issues);
        }
        if next == token {
            return Err(
                "Jira search repeated the same nextPageToken; aborting to avoid looping".into(),
            );
        }
        token = next;
    }
    Err(format!(
        "Jira search exceeded {max_pages} pages ({} issues) for this range; narrow the range",
        issues.len()
    ))
}

/// Walk `GET /issue/{key}/changelog` pages. `fetch_page` receives `startAt` and
/// returns the raw JSON body. Stops on `isLast: true`, an empty page, or when
/// `startAt + returned >= total`. Returns `Err` when `max_pages` is exhausted.
fn walk_changelog_pages<F>(
    max_pages: usize,
    mut fetch_page: F,
) -> Result<Vec<ChangelogEntry>, String>
where
    F: FnMut(u64) -> Result<String, String>,
{
    let mut entries: Vec<ChangelogEntry> = Vec::new();
    let mut start_at: u64 = 0;
    for page in 1..=max_pages {
        let body = fetch_page(start_at)?;
        let parsed: ChangelogPage = serde_json::from_str(&body)
            .map_err(|e| format!("Jira changelog JSON (page {page}): {e}"))?;
        let got = parsed.values.len() as u64;
        entries.extend(parsed.values);
        let reached_total = parsed.total.is_some_and(|t| start_at + got >= t);
        if parsed.is_last.unwrap_or(false) || got == 0 || reached_total {
            return Ok(entries);
        }
        start_at += got;
    }
    Err(format!(
        "Jira changelog exceeded {max_pages} pages ({} entries)",
        entries.len()
    ))
}

/// The inline `expand=changelog` block is complete when Jira reports a `total`
/// no larger than the histories it actually returned. A missing `total` means we
/// cannot tell, so we treat it as truncated and page through the endpoint.
fn inline_changelog_is_complete(inline: &InlineChangelog) -> bool {
    inline
        .total
        .is_some_and(|t| t <= inline.histories.len() as u64)
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    issues: Vec<JiraIssue>,
    #[serde(default, rename = "nextPageToken")]
    next_page_token: Option<String>,
    #[serde(default, rename = "isLast")]
    is_last: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct JiraIssue {
    key: String,
    fields: JiraFields,
    #[serde(default)]
    changelog: Option<InlineChangelog>,
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

/// `expand=changelog` block on a search-result issue (newest first, may be truncated).
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct InlineChangelog {
    histories: Vec<ChangelogEntry>,
    total: Option<u64>,
}

/// One page of `GET /issue/{key}/changelog` (oldest first).
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ChangelogPage {
    values: Vec<ChangelogEntry>,
    total: Option<u64>,
    #[serde(rename = "isLast")]
    is_last: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ChangelogEntry {
    author: Option<JiraAccountUser>,
    created: Option<String>,
    items: Vec<ChangeItem>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ChangeItem {
    field: Option<String>,
    #[serde(rename = "fieldId")]
    field_id: Option<String>,
}

impl ChangeItem {
    fn is_status(&self) -> bool {
        self.field_id.as_deref().is_some_and(|f| f.eq_ignore_ascii_case("status"))
            || self.field.as_deref().is_some_and(|f| f.eq_ignore_ascii_case("status"))
    }
}

/// What the user did to an issue. `label()` is part of the event id
/// (`jira:{key}:{day}:{label}`), so labels must stay stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum JiraAction {
    Created,
    Commented,
    Transitioned,
    Edited,
}

impl JiraAction {
    fn label(self) -> &'static str {
        match self {
            JiraAction::Created => "Created",
            JiraAction::Commented => "Commented",
            JiraAction::Transitioned => "Transitioned",
            JiraAction::Edited => "Edited",
        }
    }
}

/// One distinct action by the user on one local day: the earliest occurrence
/// and how many times it happened that day.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ActionOnDay {
    day: NaiveDate,
    action: JiraAction,
    first_ts: i64,
    count: usize,
}

fn local_day_and_ts(raw: &str) -> Option<(NaiveDate, i64)> {
    let dt = parse_jira_updated(raw).ok()?;
    let local = dt.with_timezone(&Local);
    Some((local.date_naive(), local.timestamp()))
}

fn is_me(user: Option<&JiraAccountUser>, my_account_id: &str) -> bool {
    user.and_then(|u| u.account_id.as_deref()) == Some(my_account_id)
}

/// Derive the user's own actions on an issue within `start_day..=end_day`
/// (local days), one entry per distinct `(day, action)`:
///
/// * comments authored by the user → `Commented` at the comment's `created`;
/// * issue created by the user → `Created` at the issue's `created`;
/// * changelog entries authored by the user → `Transitioned` when a `status`
///   item is present, otherwise `Edited`, at the entry's `created`.
///
/// If none of those fall inside the range (comments truncated by search,
/// changelog unavailable, …) fall back to a single `Edited` at the issue's
/// `updated` timestamp when that is in range — the pre-changelog behaviour —
/// so an issue Jira says you touched never disappears from the timeline.
fn user_actions(
    my_account_id: &str,
    start_day: NaiveDate,
    end_day: NaiveDate,
    fields: &JiraFields,
    changelog: &[ChangelogEntry],
) -> Vec<ActionOnDay> {
    let in_range = |day: NaiveDate| day >= start_day && day <= end_day;
    let mut raw: Vec<(NaiveDate, i64, JiraAction)> = Vec::new();

    if let Some(block) = &fields.comment {
        for c in &block.comments {
            if !is_me(c.author.as_ref(), my_account_id) {
                continue;
            }
            if let Some((day, ts)) = c.created.as_deref().and_then(local_day_and_ts) {
                if in_range(day) {
                    raw.push((day, ts, JiraAction::Commented));
                }
            }
        }
    }

    if is_me(fields.creator.as_ref(), my_account_id) {
        if let Some((day, ts)) = fields.created.as_deref().and_then(local_day_and_ts) {
            if in_range(day) {
                raw.push((day, ts, JiraAction::Created));
            }
        }
    }

    for entry in changelog {
        if !is_me(entry.author.as_ref(), my_account_id) {
            continue;
        }
        let Some((day, ts)) = entry.created.as_deref().and_then(local_day_and_ts) else {
            continue;
        };
        if !in_range(day) {
            continue;
        }
        let action = if entry.items.iter().any(ChangeItem::is_status) {
            JiraAction::Transitioned
        } else {
            JiraAction::Edited
        };
        raw.push((day, ts, action));
    }

    if raw.is_empty() {
        if let Some((day, ts)) = local_day_and_ts(&fields.updated) {
            if in_range(day) {
                raw.push((day, ts, JiraAction::Edited));
            }
        }
    }

    let mut grouped: BTreeMap<(NaiveDate, JiraAction), (i64, usize)> = BTreeMap::new();
    for (day, ts, action) in raw {
        let slot = grouped.entry((day, action)).or_insert((ts, 0));
        slot.0 = slot.0.min(ts);
        slot.1 += 1;
    }
    let mut out: Vec<ActionOnDay> = grouped
        .into_iter()
        .map(|((day, action), (first_ts, count))| ActionOnDay { day, action, first_ts, count })
        .collect();
    out.sort_by_key(|a| (a.first_ts, a.action));
    out
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

    // --- search request body ---

    #[test]
    fn search_request_body_omits_token_on_first_page() {
        let body = search_request_body("project = X", None).unwrap();
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["jql"], "project = X");
        assert_eq!(v["maxResults"], SEARCH_PAGE_SIZE);
        assert_eq!(v["expand"], "changelog");
        assert!(v.get("nextPageToken").is_none(), "first page must not send a token: {body}");
    }

    #[test]
    fn search_request_body_echoes_token_on_later_pages() {
        let body = search_request_body("project = X", Some("tok-2")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["nextPageToken"], "tok-2");
    }

    // --- search pagination ---

    fn issue_json(key: &str) -> String {
        format!(
            r#"{{"key":"{key}","fields":{{"summary":"s","updated":"2024-01-15T12:00:00.000+0000"}}}}"#
        )
    }

    fn page_json(keys: &[&str], next: Option<&str>, is_last: Option<bool>) -> String {
        let issues: Vec<String> = keys.iter().map(|k| issue_json(k)).collect();
        let next = match next {
            Some(t) => format!("\"{t}\""),
            None => "null".to_string(),
        };
        let is_last = match is_last {
            Some(b) => format!(",\"isLast\":{b}"),
            None => String::new(),
        };
        format!(
            "{{\"issues\":[{}],\"nextPageToken\":{next}{is_last}}}",
            issues.join(",")
        )
    }

    #[test]
    fn walk_search_pages_follows_tokens_until_null() {
        let mut seen: Vec<Option<String>> = Vec::new();
        let issues = walk_search_pages(10, |tok| {
            seen.push(tok.map(str::to_string));
            Ok(match tok {
                None => page_json(&["A-1", "A-2"], Some("t1"), Some(false)),
                Some("t1") => page_json(&["A-3"], Some("t2"), Some(false)),
                Some("t2") => page_json(&["A-4"], None, Some(true)),
                other => panic!("unexpected token {other:?}"),
            })
        })
        .unwrap();
        let keys: Vec<&str> = issues.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(keys, ["A-1", "A-2", "A-3", "A-4"]);
        assert_eq!(
            seen,
            vec![None, Some("t1".to_string()), Some("t2".to_string())]
        );
    }

    #[test]
    fn walk_search_pages_stops_when_token_absent_even_without_is_last() {
        let mut calls = 0;
        let issues = walk_search_pages(10, |_| {
            calls += 1;
            Ok(r#"{"issues":[{"key":"B-1","fields":{"summary":"s","updated":"2024-01-15T12:00:00.000+0000"}}]}"#.to_string())
        })
        .unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(calls, 1);
    }

    #[test]
    fn walk_search_pages_honours_is_last_true_over_token() {
        let mut calls = 0;
        let issues = walk_search_pages(10, |_| {
            calls += 1;
            Ok(page_json(&["C-1"], Some("stale"), Some(true)))
        })
        .unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(calls, 1);
    }

    #[test]
    fn walk_search_pages_errs_when_cap_hit() {
        let mut calls = 0;
        let res = walk_search_pages(3, |tok| {
            calls += 1;
            let next = format!("t{calls}");
            Ok(page_json(&["D-1"], Some(&next), Some(false)).replace("D-1", &format!("D-{}", tok.map_or(0, |_| calls))))
        });
        let err = res.unwrap_err();
        assert!(err.contains("exceeded 3 pages"), "{err}");
        assert_eq!(calls, 3, "must not fetch past the cap");
    }

    #[test]
    fn walk_search_pages_errs_on_repeated_token() {
        let res = walk_search_pages(10, |_| Ok(page_json(&["E-1"], Some("same"), Some(false))));
        let err = res.unwrap_err();
        assert!(err.contains("repeated"), "{err}");
    }

    #[test]
    fn walk_search_pages_propagates_fetch_error() {
        let res = walk_search_pages(10, |tok| match tok {
            None => Ok(page_json(&["F-1"], Some("t1"), Some(false))),
            Some(_) => Err("HTTP 429".to_string()),
        });
        assert_eq!(res.unwrap_err(), "HTTP 429");
    }

    #[test]
    fn walk_search_pages_errs_on_bad_json() {
        let res = walk_search_pages(10, |_| Ok("not json".to_string()));
        assert!(res.unwrap_err().contains("Jira search JSON"));
    }

    // --- changelog pagination ---

    fn changelog_page(n: usize, total: Option<u64>, is_last: Option<bool>) -> String {
        let values: Vec<String> = (0..n)
            .map(|_| r#"{"author":{"accountId":"me"},"created":"2024-01-15T12:00:00.000+0000","items":[]}"#.to_string())
            .collect();
        let mut s = format!("{{\"values\":[{}]", values.join(","));
        if let Some(t) = total {
            s.push_str(&format!(",\"total\":{t}"));
        }
        if let Some(b) = is_last {
            s.push_str(&format!(",\"isLast\":{b}"));
        }
        s.push('}');
        s
    }

    #[test]
    fn walk_changelog_pages_offsets_by_returned_count() {
        let mut offsets = Vec::new();
        let entries = walk_changelog_pages(10, |start_at| {
            offsets.push(start_at);
            Ok(match start_at {
                0 => changelog_page(2, Some(5), Some(false)),
                2 => changelog_page(2, Some(5), Some(false)),
                4 => changelog_page(1, Some(5), Some(true)),
                other => panic!("unexpected startAt {other}"),
            })
        })
        .unwrap();
        assert_eq!(entries.len(), 5);
        assert_eq!(offsets, vec![0, 2, 4]);
    }

    #[test]
    fn walk_changelog_pages_stops_at_total_without_is_last() {
        let mut calls = 0;
        let entries = walk_changelog_pages(10, |_| {
            calls += 1;
            Ok(changelog_page(3, Some(3), None))
        })
        .unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(calls, 1);
    }

    #[test]
    fn walk_changelog_pages_stops_on_empty_page() {
        let mut calls = 0;
        let entries = walk_changelog_pages(10, |_| {
            calls += 1;
            Ok(changelog_page(0, None, None))
        })
        .unwrap();
        assert!(entries.is_empty());
        assert_eq!(calls, 1);
    }

    #[test]
    fn walk_changelog_pages_errs_when_cap_hit() {
        let mut calls = 0;
        let res = walk_changelog_pages(2, |_| {
            calls += 1;
            Ok(changelog_page(1, Some(1000), Some(false)))
        });
        assert!(res.unwrap_err().contains("exceeded 2 pages"));
        assert_eq!(calls, 2);
    }

    #[test]
    fn inline_changelog_complete_only_when_total_covered() {
        let entry = || ChangelogEntry::default();
        assert!(inline_changelog_is_complete(&InlineChangelog {
            histories: vec![entry(), entry()],
            total: Some(2),
        }));
        assert!(!inline_changelog_is_complete(&InlineChangelog {
            histories: vec![entry()],
            total: Some(2),
        }));
        assert!(!inline_changelog_is_complete(&InlineChangelog {
            histories: vec![entry()],
            total: None,
        }));
        assert!(inline_changelog_is_complete(&InlineChangelog {
            histories: vec![],
            total: Some(0),
        }));
    }

    // --- timestamp / action selection ---

    const ME: &str = "user123";
    const OTHER: &str = "someone-else";

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 1, d).unwrap()
    }

    /// Midday UTC on 2024-01-{d} plus `h` hours: the same local date for every
    /// offset within ±11h, so tests are independent of the machine's timezone.
    fn at(d: u32, h: i64) -> String {
        format!("2024-01-{d:02}T{:02}:00:00.000+0000", 12 + h)
    }

    fn ts_of(s: &str) -> i64 {
        parse_jira_updated(s).unwrap().timestamp()
    }

    fn user(id: &str) -> Option<JiraAccountUser> {
        Some(JiraAccountUser { account_id: Some(id.to_string()) })
    }

    fn fields(updated: &str) -> JiraFields {
        JiraFields {
            summary: "Test issue".to_string(),
            updated: updated.to_string(),
            created: None,
            creator: None,
            comment: None,
        }
    }

    fn comment(author: &str, created: &str) -> JiraComment {
        JiraComment { author: user(author), created: Some(created.to_string()) }
    }

    fn history(author: &str, created: &str, field_ids: &[&str]) -> ChangelogEntry {
        ChangelogEntry {
            author: user(author),
            created: Some(created.to_string()),
            items: field_ids
                .iter()
                .map(|f| ChangeItem { field: None, field_id: Some(f.to_string()) })
                .collect(),
        }
    }

    #[test]
    fn user_actions_uses_own_comment_time_not_issue_updated() {
        // I commented at 12:00; a colleague edited at 16:00 (fields.updated).
        let mut f = fields(&at(15, 4));
        f.comment = Some(JiraCommentBlock { comments: vec![comment(ME, &at(15, 0))] });
        let actions = user_actions(ME, day(15), day(15), &f, &[]);
        assert_eq!(
            actions,
            vec![ActionOnDay { day: day(15), action: JiraAction::Commented, first_ts: ts_of(&at(15, 0)), count: 1 }]
        );
    }

    #[test]
    fn user_actions_ignores_other_peoples_comments_and_changes() {
        let mut f = fields(&at(15, 4));
        f.comment = Some(JiraCommentBlock { comments: vec![comment(OTHER, &at(15, 0))] });
        let changelog = vec![history(OTHER, &at(15, 1), &["status"])];
        let actions = user_actions(ME, day(15), day(15), &f, &changelog);
        // Nothing authored by me → fallback to `updated` as Edited.
        assert_eq!(
            actions,
            vec![ActionOnDay { day: day(15), action: JiraAction::Edited, first_ts: ts_of(&at(15, 4)), count: 1 }]
        );
    }

    #[test]
    fn user_actions_transition_from_changelog_status_item() {
        let f = fields(&at(15, 4));
        let changelog = vec![history(ME, &at(15, 1), &["assignee", "status"])];
        let actions = user_actions(ME, day(15), day(15), &f, &changelog);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action, JiraAction::Transitioned);
        assert_eq!(actions[0].first_ts, ts_of(&at(15, 1)));
    }

    #[test]
    fn user_actions_status_matched_by_field_name_too() {
        let f = fields(&at(15, 4));
        let changelog = vec![ChangelogEntry {
            author: user(ME),
            created: Some(at(15, 1)),
            items: vec![ChangeItem { field: Some("Status".to_string()), field_id: None }],
        }];
        let actions = user_actions(ME, day(15), day(15), &f, &changelog);
        assert_eq!(actions[0].action, JiraAction::Transitioned);
    }

    #[test]
    fn user_actions_field_edit_from_changelog() {
        let f = fields(&at(15, 4));
        let changelog = vec![history(ME, &at(15, 2), &["description"])];
        let actions = user_actions(ME, day(15), day(15), &f, &changelog);
        assert_eq!(
            actions,
            vec![ActionOnDay { day: day(15), action: JiraAction::Edited, first_ts: ts_of(&at(15, 2)), count: 1 }]
        );
    }

    #[test]
    fn user_actions_one_event_per_distinct_action_with_earliest_time_and_count() {
        let mut f = fields(&at(15, 5));
        f.comment = Some(JiraCommentBlock {
            comments: vec![comment(ME, &at(15, 3)), comment(ME, &at(15, 0)), comment(ME, &at(15, 1))],
        });
        let changelog = vec![
            history(ME, &at(15, 2), &["status"]),
            history(ME, &at(15, 4), &["status"]),
            history(ME, &at(15, -1), &["priority"]),
        ];
        let actions = user_actions(ME, day(15), day(15), &f, &changelog);
        assert_eq!(
            actions,
            vec![
                ActionOnDay { day: day(15), action: JiraAction::Edited, first_ts: ts_of(&at(15, -1)), count: 1 },
                ActionOnDay { day: day(15), action: JiraAction::Commented, first_ts: ts_of(&at(15, 0)), count: 3 },
                ActionOnDay { day: day(15), action: JiraAction::Transitioned, first_ts: ts_of(&at(15, 2)), count: 2 },
            ]
        );
    }

    #[test]
    fn user_actions_splits_by_day_and_filters_outside_range() {
        let mut f = fields(&at(20, 0));
        f.comment = Some(JiraCommentBlock {
            comments: vec![comment(ME, &at(14, 0)), comment(ME, &at(15, 0)), comment(ME, &at(16, 0)), comment(ME, &at(19, 0))],
        });
        let actions = user_actions(ME, day(15), day(16), &f, &[]);
        let days: Vec<NaiveDate> = actions.iter().map(|a| a.day).collect();
        assert_eq!(days, vec![day(15), day(16)]);
        assert!(actions.iter().all(|a| a.action == JiraAction::Commented && a.count == 1));
    }

    #[test]
    fn user_actions_created_by_me() {
        let mut f = fields(&at(15, 4));
        f.created = Some(at(15, 0));
        f.creator = user(ME);
        let actions = user_actions(ME, day(15), day(15), &f, &[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action, JiraAction::Created);
        assert_eq!(actions[0].first_ts, ts_of(&at(15, 0)));
    }

    #[test]
    fn user_actions_created_by_other_is_not_mine() {
        let mut f = fields(&at(15, 4));
        f.created = Some(at(15, 0));
        f.creator = user(OTHER);
        let actions = user_actions(ME, day(15), day(15), &f, &[]);
        assert_eq!(actions[0].action, JiraAction::Edited);
        assert_eq!(actions[0].first_ts, ts_of(&at(15, 4)));
    }

    #[test]
    fn user_actions_no_fallback_when_own_action_found_on_other_in_range_day() {
        // Commented on the 15th; colleague's edit moved `updated` to the 16th.
        // Only my comment must be emitted — not a phantom "Edited" on the 16th.
        let mut f = fields(&at(16, 0));
        f.comment = Some(JiraCommentBlock { comments: vec![comment(ME, &at(15, 0))] });
        let actions = user_actions(ME, day(15), day(16), &f, &[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].day, day(15));
        assert_eq!(actions[0].action, JiraAction::Commented);
    }

    #[test]
    fn user_actions_fallback_skipped_when_updated_out_of_range() {
        let f = fields(&at(20, 0));
        assert!(user_actions(ME, day(15), day(15), &f, &[]).is_empty());
    }

    #[test]
    fn user_actions_skips_unparseable_timestamps() {
        let mut f = fields(&at(15, 4));
        f.comment = Some(JiraCommentBlock { comments: vec![comment(ME, "garbage")] });
        let changelog = vec![history(ME, "also garbage", &["status"])];
        let actions = user_actions(ME, day(15), day(15), &f, &changelog);
        assert_eq!(actions[0].action, JiraAction::Edited);
    }

    #[test]
    fn action_labels_are_stable_for_event_ids() {
        assert_eq!(JiraAction::Created.label(), "Created");
        assert_eq!(JiraAction::Commented.label(), "Commented");
        assert_eq!(JiraAction::Transitioned.label(), "Transitioned");
        assert_eq!(JiraAction::Edited.label(), "Edited");
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
