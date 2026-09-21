//! GitHub timeline source.
//!
//! # Why the REST Search API (and not only the Events API, nor GraphQL)
//!
//! `GET /users/{username}/events` only returns the ~300 most recent events,
//! and none older than 90 days. For an active developer that is about one
//! week (issue #101), so it cannot answer "what did I do on <date>" for
//! arbitrary dates. Two range-capable alternatives were evaluated against
//! the GitHub docs (September 2026):
//!
//! * GraphQL `contributionsCollection(from:, to:)` has `occurredAt` for PRs
//!   opened, issues opened and reviews, but carries no comments at all, only
//!   the *most recent* review per PR, and commits only as per-day counts per
//!   repository without branch, message or time of day. Three of the six
//!   opt-in event types would be lost outright.
//! * REST Search (`/search/issues` with `author:`, `commenter:`,
//!   `reviewed-by:` and `created:`/`updated:` qualifiers, plus
//!   `/search/commits`) supports arbitrary date ranges, up to 1,000 results
//!   per query at 30 requests/minute. It does not carry per-comment or
//!   per-review timestamps, so every candidate issue/PR found by
//!   `commenter:`/`reviewed-by:` is followed up through the ordinary REST
//!   list endpoints (`…/comments?since=`, `…/reviews`), which do return exact
//!   timestamps and the numeric ids GitHub uses in its `#issuecomment-`,
//!   `#pullrequestreview-` and `#discussion_r` anchors. URLs therefore stay
//!   byte-identical to what the Events API version produced.
//!
//! Search is used for everything except pushes. Push events (branch, push
//! time, commits on *any* branch) exist only in the Events API, so pushes
//! keep coming from there for as far back as it reaches, and
//! `/search/commits` (default branch only, by author date) fills in the days
//! beyond that horizon.
//!
//! `advanced_search=true` is passed explicitly on `/search/issues`: it became
//! the default on 2025-09-04, and its `AND`/`OR` semantics are what the
//! combined candidate query relies on.
//!
//! Search requests are the scarce resource (30/minute shared across the
//! app), so a range fetch issues at most three of them: one for things the
//! user opened, one for things they commented on or reviewed, and — only
//! when the Events API does not cover the whole range — one commit search.
//! Follow-up list requests draw on the ordinary 5,000/hour budget and run on
//! a small thread pool.

use std::collections::HashSet;

use chrono::{DateTime, Duration, Local, NaiveDate, SecondsFormat, TimeZone, Utc};
use serde::Deserialize;
use serde_json::Value;
use tauri::State;

use crate::commands::settings_github::{get_settings_github, GitHubEvent};
use crate::state::AppState;
use crate::timeline::{sanitize_event_url, TimelineEvent, TimelineEventSource};

/// One timeline row bucketed by local day: `(day, unix timestamp, event)`.
type Row = (NaiveDate, i64, TimelineEvent);

const API: &str = "https://api.github.com";
const USER_AGENT: &str = "recall-app";
const PER_PAGE: usize = 100;
/// GitHub search never pages past this many results, however large
/// `total_count` is. Hitting it means the range is incomplete.
const SEARCH_RESULT_CAP: usize = 1000;
/// The Events API only returns events created within this many days.
const EVENTS_API_MAX_AGE_DAYS: i64 = 90;
/// Concurrent follow-up (comments / reviews) requests.
const FOLLOW_UP_THREADS: usize = 6;

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

/// Events for one local day. The `bool` is true when GitHub could not return
/// complete data for the day (a search result cap was hit); such a day must
/// not be cached.
pub(super) fn events_for_day(
    state: &State<'_, AppState>,
    day: &str,
) -> Result<(Vec<(i64, TimelineEvent)>, bool), String> {
    let day_naive =
        NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|_| format!("Invalid date: {day}"))?;
    let (rows, incomplete) = events_for_range(state, day_naive, day_naive)?;
    Ok((
        rows.into_iter().map(|(_, ts, ev)| (ts, ev)).collect(),
        !incomplete.is_empty(),
    ))
}

/// Events for every local day in `start_day..=end_day`, bucketed by day, plus
/// the days for which GitHub could not return complete data. Callers must not
/// cache those days: the missing events would never be re-fetched.
pub(super) fn events_for_range(
    state: &State<'_, AppState>,
    start_day: NaiveDate,
    end_day: NaiveDate,
) -> Result<(Vec<Row>, Vec<NaiveDate>), String> {
    let Some(settings) = get_settings_github(state.clone()) else {
        return Ok((Vec::new(), Vec::new()));
    };
    if !settings.enabled {
        return Ok((Vec::new(), Vec::new()));
    }
    let user = settings.username.trim();
    if user.is_empty() || settings.token.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    if settings.enabled_events.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    // The username is interpolated into search qualifiers, so refuse anything
    // that could smuggle in extra qualifiers.
    validate_username(user)?;

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
    // until is the exclusive upper bound (midnight after end_day).
    let range = Range {
        since: since_local.with_timezone(&Utc),
        until: until_local.with_timezone(&Utc),
    };

    let client = GhClient::new(&settings.token);
    let enabled = |e: GitHubEvent| settings.enabled_events.contains(&e);
    let mut rows: Vec<Row> = Vec::new();
    let mut complete = true;

    // --- PRs and issues the user opened (exact created_at from search) ---
    if let Some(query) = opened_query(
        user,
        enabled(GitHubEvent::PullRequestEvent),
        enabled(GitHubEvent::IssuesEvent),
        &range,
    ) {
        let found = search(&client, "issues", &query)?;
        complete &= found.complete;
        rows.extend(found.items.iter().filter_map(|i| map_opened(i, &range)));
    }

    // --- Comments, reviews and review comments ---
    // Search only tells us *which* issues/PRs the user touched, so each
    // candidate is followed up with the list endpoints for exact timestamps.
    let want_comments = enabled(GitHubEvent::IssueCommentEvent);
    let want_reviews = enabled(GitHubEvent::PullRequestReviewEvent);
    let want_review_comments = enabled(GitHubEvent::PullRequestReviewCommentEvent);
    if let Some(query) = candidate_query(
        user,
        want_comments,
        want_reviews || want_review_comments,
        &range,
    ) {
        let found = search(&client, "issues", &query)?;
        complete &= found.complete;
        let candidates: Vec<IssueRef> = found.items.iter().filter_map(issue_ref).collect();
        let fetched = parallel_fetch(candidates, |c| {
            let mut out = Vec::new();
            if want_comments {
                out.extend(fetch_issue_comments(&client, &c, user, &range)?);
            }
            if c.is_pr && want_reviews {
                out.extend(fetch_pr_reviews(&client, &c, user, &range)?);
            }
            if c.is_pr && want_review_comments {
                out.extend(fetch_pr_review_comments(&client, &c, user, &range)?);
            }
            Ok(out)
        })?;
        rows.extend(fetched);
    }

    // --- Pushes: Events API while it reaches, commit search beyond ---
    if enabled(GitHubEvent::PushEvent) {
        let (events, reached_since) = rest_api_user_events(&client, user, range.since.timestamp())?;
        let oldest = events
            .iter()
            .filter_map(|e| parse_github_datetime(&e.created_at).ok())
            .min();
        let horizon = push_horizon(reached_since, oldest, &range, Utc::now());

        let mut pushed_shas: HashSet<String> = HashSet::new();
        for ev in events.iter().filter(|e| e.event_type == "PushEvent") {
            let created = parse_github_datetime(&ev.created_at)?;
            if created < horizon || !range.contains(created) {
                continue;
            }
            pushed_shas.extend(push_event_shas(ev));
            if let Some(mapped) = map_push_event(ev) {
                // Ids for push rows are unchanged from the Events-API-only
                // implementation, so existing Harvest checkmarks stay valid.
                let id = format!("github:{}", github_event_id_str(&ev.id));
                rows.push(make_row(id, created, mapped.title, mapped.detail, mapped.url));
            }
        }

        if horizon > range.since {
            let fallback = Range {
                since: range.since,
                until: horizon.min(range.until),
            };
            let query = format!(
                "author:{user} author-date:{}",
                fallback.created_qualifier()
            );
            let found = search(&client, "commits", &query)?;
            complete &= found.complete;
            rows.extend(
                found
                    .items
                    .iter()
                    .filter_map(|i| map_commit(i, &fallback, &pushed_shas)),
            );
        }
    }

    rows.sort_by_key(|(_, ts, _)| *ts);
    let incomplete = if complete {
        Vec::new()
    } else {
        days_between(start_day, end_day)
    };
    Ok((rows, incomplete))
}

// ---------------------------------------------------------------------------
// Range helpers
// ---------------------------------------------------------------------------

/// A UTC time range; `until` is exclusive.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Range {
    since: DateTime<Utc>,
    until: DateTime<Utc>,
}

impl Range {
    fn contains(&self, t: DateTime<Utc>) -> bool {
        t >= self.since && t < self.until
    }

    /// `A..B` for `created:` / `author-date:`. Search ranges are inclusive at
    /// both ends, so the exclusive `until` is pulled back one second.
    fn created_qualifier(&self) -> String {
        format!(
            "{}..{}",
            fmt_search_time(self.since),
            fmt_search_time(self.until - Duration::seconds(1))
        )
    }

    /// `>=A` for `updated:`. Deliberately open-ended: an issue commented on
    /// inside the range may have been updated again long after it.
    fn since_qualifier(&self) -> String {
        format!(">={}", fmt_search_time(self.since))
    }

    /// Value for the `since=` query parameter of REST list endpoints.
    fn since_param(&self) -> String {
        self.since.to_rfc3339_opts(SecondsFormat::Secs, true)
    }
}

/// Search qualifiers accept ISO 8601 with an explicit offset; GitHub's own
/// examples use `+00:00` rather than `Z`.
fn fmt_search_time(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%dT%H:%M:%S+00:00").to_string()
}

fn days_between(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let mut days = Vec::new();
    let mut d = start;
    while d <= end {
        days.push(d);
        let Some(next) = d.succ_opt() else { break };
        d = next;
    }
    days
}

/// GitHub usernames are alphanumerics and hyphens. Anything else could inject
/// extra qualifiers into a search query, so refuse it.
fn validate_username(user: &str) -> Result<(), String> {
    if !user.is_empty()
        && user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        Ok(())
    } else {
        Err(format!("Invalid GitHub username {user:?}"))
    }
}

/// Search query for PRs and/or issues the user opened inside `range`, or
/// `None` when neither event type is enabled.
fn opened_query(user: &str, prs: bool, issues: bool, range: &Range) -> Option<String> {
    let kind = match (prs, issues) {
        (true, true) => "",
        (true, false) => "is:pr ",
        (false, true) => "is:issue ",
        (false, false) => return None,
    };
    Some(format!(
        "{kind}author:{user} created:{}",
        range.created_qualifier()
    ))
}

/// Search query for issues/PRs the user commented on and/or reviewed that saw
/// any activity since the range started. One query covers both roles (`OR`
/// needs advanced search) to conserve the 30/minute search budget.
fn candidate_query(user: &str, comments: bool, reviews: bool, range: &Range) -> Option<String> {
    let who = match (comments, reviews) {
        (true, true) => format!("(commenter:{user} OR reviewed-by:{user})"),
        (true, false) => format!("commenter:{user}"),
        (false, true) => format!("is:pr reviewed-by:{user}"),
        (false, false) => return None,
    };
    Some(format!("{who} updated:{}", range.since_qualifier()))
}

/// Earliest instant for which push data from the Events API is known to be
/// complete. Before it, `/search/commits` must fill in.
///
/// * `reached_since`: pagination stopped because it saw an event older than
///   the range start, so everything from `range.since` on was returned.
/// * otherwise the API ran dry: only events at or after the oldest one
///   returned are complete (none at all → nothing is).
/// * never earlier than the API's 90-day floor, and never earlier than the
///   range itself.
fn push_horizon(
    reached_since: bool,
    oldest: Option<DateTime<Utc>>,
    range: &Range,
    now: DateTime<Utc>,
) -> DateTime<Utc> {
    let api_floor = now - Duration::days(EVENTS_API_MAX_AGE_DAYS);
    let horizon = if reached_since {
        range.since
    } else {
        oldest.unwrap_or(api_floor)
    };
    horizon.max(api_floor).max(range.since)
}

// ---------------------------------------------------------------------------
// HTTP
// ---------------------------------------------------------------------------

struct GhClient {
    auth: String,
}

impl GhClient {
    fn new(token: &str) -> Self {
        Self {
            auth: format!("Bearer {token}"),
        }
    }

    /// GET `url` and return `(status, body)`. Non-2xx statuses are returned,
    /// not raised, so callers can treat e.g. 404 or 422 as "no more data";
    /// transport failures and exhausted rate limits are errors.
    fn get(&self, url: &str) -> Result<(u16, Value), String> {
        let mut response = ureq::get(url)
            .config()
            .http_status_as_error(false)
            .build()
            .header("Authorization", &self.auth)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", USER_AGENT)
            .call()
            .map_err(|e| format!("GitHub API request failed: {e}"))?;
        let status = response.status().as_u16();
        let remaining = response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body: Value = response.body_mut().read_json().unwrap_or(Value::Null);
        if (status == 403 || status == 429) && remaining.as_deref() == Some("0") {
            return Err(
                "GitHub API rate limit reached — wait a minute and try again".to_string(),
            );
        }
        Ok((status, body))
    }

    /// GET `url`, treating any non-2xx status as an error.
    fn get_json(&self, url: &str, label: &str) -> Result<Value, String> {
        let (status, body) = self.get(url)?;
        if status >= 400 {
            return Err(api_error(status, &body, label));
        }
        Ok(body)
    }
}

fn api_error(status: u16, body: &Value, label: &str) -> String {
    let message = j_str(body, &["message"])
        .map(|m| format!(": {m}"))
        .unwrap_or_default();
    match status {
        401 | 403 => format!(
            "GitHub API returned HTTP {status} for {label} (check your username and token){message}"
        ),
        _ => format!("GitHub API returned HTTP {status} for {label}{message}"),
    }
}

struct SearchResult {
    items: Vec<Value>,
    /// False when GitHub said `incomplete_results` or the 1,000-result cap
    /// prevented paging through everything.
    complete: bool,
}

fn search_url(endpoint: &str, query: &str, page: usize) -> String {
    let mut url = format!(
        "{API}/search/{endpoint}?q={}&per_page={PER_PAGE}&page={page}",
        urlencoding::encode(query)
    );
    if endpoint == "issues" {
        url.push_str("&advanced_search=true");
    }
    url
}

/// Run one search query and page through all of its results (up to the cap).
fn search(client: &GhClient, endpoint: &str, query: &str) -> Result<SearchResult, String> {
    let label = format!("search/{endpoint}");
    let mut items: Vec<Value> = Vec::new();
    let mut complete = true;
    for page in 1.. {
        let body = client.get_json(&search_url(endpoint, query, page), &label)?;
        let total = body
            .get("total_count")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        if body.get("incomplete_results").and_then(Value::as_bool) == Some(true) {
            complete = false;
        }
        let page_items = body
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let got = page_items.len();
        items.extend(page_items);
        if got < PER_PAGE || items.len() >= total {
            break;
        }
        if items.len() >= SEARCH_RESULT_CAP {
            complete = false;
            break;
        }
    }
    Ok(SearchResult { items, complete })
}

/// Fetch every page of a REST list endpoint. 404/410 mean the item vanished
/// between the search and now (deleted, or the repo became inaccessible), so
/// they yield an empty list rather than failing the whole range.
fn list_all(client: &GhClient, url: &str, label: &str) -> Result<Vec<Value>, String> {
    let sep = if url.contains('?') { '&' } else { '?' };
    let mut items: Vec<Value> = Vec::new();
    for page in 1..=50 {
        let page_url = format!("{url}{sep}per_page={PER_PAGE}&page={page}");
        let (status, body) = client.get(&page_url)?;
        if status == 404 || status == 410 {
            return Ok(Vec::new());
        }
        if status >= 400 {
            return Err(api_error(status, &body, label));
        }
        let page_items = body.as_array().cloned().unwrap_or_default();
        let got = page_items.len();
        items.extend(page_items);
        if got < PER_PAGE {
            break;
        }
    }
    Ok(items)
}

/// Run `f` over `items` on a handful of threads and concatenate the rows.
/// Follow-up requests are independent HTTP calls; serialized, a month with a
/// couple of hundred candidates would take minutes.
fn parallel_fetch<T: Send>(
    items: Vec<T>,
    f: impl Fn(T) -> Result<Vec<Row>, String> + Sync,
) -> Result<Vec<Row>, String> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let threads = FOLLOW_UP_THREADS.min(items.len());
    let chunk_size = items.len().div_ceil(threads);
    let mut chunks: Vec<Vec<T>> = Vec::with_capacity(threads);
    let mut iter = items.into_iter();
    loop {
        let chunk: Vec<T> = iter.by_ref().take(chunk_size).collect();
        if chunk.is_empty() {
            break;
        }
        chunks.push(chunk);
    }
    let f = &f;
    std::thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                scope.spawn(move || {
                    let mut out: Vec<Row> = Vec::new();
                    for item in chunk {
                        out.extend(f(item)?);
                    }
                    Ok::<Vec<Row>, String>(out)
                })
            })
            .collect();
        let mut rows = Vec::new();
        for handle in handles {
            rows.extend(
                handle
                    .join()
                    .map_err(|_| "GitHub fetch thread panicked".to_string())??,
            );
        }
        Ok(rows)
    })
}

/// Page through `GET /users/{username}/events` (newest first). Returns the
/// events and whether pagination reached an event older than `since_ts` —
/// i.e. whether everything from `since_ts` on is present. When it returns
/// `false` the API ran dry (300-event cap / 90-day window) before reaching the
/// range start.
fn rest_api_user_events(
    client: &GhClient,
    username: &str,
    since_ts: i64,
) -> Result<(Vec<GhEvent>, bool), String> {
    let mut all_events: Vec<GhEvent> = Vec::new();

    for page in 1u32..=10 {
        let url = format!(
            "{API}/users/{}/events?per_page={PER_PAGE}&page={page}",
            urlencoding::encode(username),
        );
        let (status, body) = client.get(&url)?;
        // 422 past page 1 means "no more pages" (the events-API page limit).
        if status == 422 && page > 1 {
            break;
        }
        if status >= 400 {
            return Err(api_error(status, &body, "user events"));
        }
        let page_events: Vec<GhEvent> = serde_json::from_value(body)
            .map_err(|e| format!("Could not parse GitHub events JSON: {e}"))?;
        if page_events.is_empty() {
            break;
        }

        // Events are returned newest-first. If the last event on this page is
        // already older than the range start, every subsequent page will be
        // older too — stop paginating to save API calls.
        let oldest_ts = page_events
            .last()
            .and_then(|e| parse_github_datetime(&e.created_at).ok())
            .map(|dt| dt.timestamp());
        all_events.extend(page_events);
        if let Some(ts) = oldest_ts {
            if ts < since_ts {
                return Ok((all_events, true));
            }
        }
    }

    Ok((all_events, false))
}

// ---------------------------------------------------------------------------
// Follow-up fetches
// ---------------------------------------------------------------------------

fn fetch_issue_comments(
    client: &GhClient,
    issue: &IssueRef,
    user: &str,
    range: &Range,
) -> Result<Vec<Row>, String> {
    let url = format!(
        "{API}/repos/{}/issues/{}/comments?since={}",
        issue.repo,
        issue.number,
        urlencoding::encode(&range.since_param())
    );
    let items = list_all(client, &url, "issue comments")?;
    Ok(items
        .iter()
        .filter_map(|c| map_issue_comment(c, issue, user, range))
        .collect())
}

fn fetch_pr_reviews(
    client: &GhClient,
    pr: &IssueRef,
    user: &str,
    range: &Range,
) -> Result<Vec<Row>, String> {
    // The reviews endpoint has no `since`; reviews per PR are few.
    let url = format!("{API}/repos/{}/pulls/{}/reviews", pr.repo, pr.number);
    let items = list_all(client, &url, "PR reviews")?;
    Ok(items
        .iter()
        .filter_map(|r| map_pr_review(r, pr, user, range))
        .collect())
}

fn fetch_pr_review_comments(
    client: &GhClient,
    pr: &IssueRef,
    user: &str,
    range: &Range,
) -> Result<Vec<Row>, String> {
    let url = format!(
        "{API}/repos/{}/pulls/{}/comments?since={}",
        pr.repo,
        pr.number,
        urlencoding::encode(&range.since_param())
    );
    let items = list_all(client, &url, "PR review comments")?;
    Ok(items
        .iter()
        .filter_map(|c| map_pr_review_comment(c, pr, user, range))
        .collect())
}

// ---------------------------------------------------------------------------
// Mapping: API objects → timeline rows
// ---------------------------------------------------------------------------

/// The issue or PR a search hit refers to.
#[derive(Debug, Clone, PartialEq)]
struct IssueRef {
    /// `owner/name`
    repo: String,
    number: u64,
    title: String,
    is_pr: bool,
}

impl IssueRef {
    /// `pull` or `issues` — the path segment GitHub uses in html URLs.
    fn path(&self) -> &'static str {
        if self.is_pr {
            "pull"
        } else {
            "issues"
        }
    }

    fn html_url(&self) -> String {
        format!("https://github.com/{}/{}/{}", self.repo, self.path(), self.number)
    }

    fn subject(&self) -> Option<String> {
        Some(self.title.clone()).filter(|s| !s.trim().is_empty())
    }
}

/// Parse a `/search/issues` item into an [`IssueRef`].
fn issue_ref(item: &Value) -> Option<IssueRef> {
    let repo = j_str(item, &["repository_url"])
        .and_then(|u| repo_from_repository_url(&u))
        .or_else(|| j_str(item, &["html_url"]).and_then(|u| repo_from_html_url(&u)))?;
    Some(IssueRef {
        repo,
        number: j_u64(item, &["number"])?,
        title: j_str(item, &["title"]).unwrap_or_default(),
        is_pr: item.get("pull_request").is_some(),
    })
}

/// `https://api.github.com/repos/owner/name` → `owner/name`
fn repo_from_repository_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix(&format!("{API}/repos/"))?;
    let mut parts = rest.split('/');
    let owner = parts.next().filter(|s| !s.is_empty())?;
    let name = parts.next().filter(|s| !s.is_empty())?;
    Some(format!("{owner}/{name}"))
}

/// `https://github.com/owner/name/pull/1` → `owner/name`
fn repo_from_html_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://github.com/")?;
    let mut parts = rest.split('/');
    let owner = parts.next().filter(|s| !s.is_empty())?;
    let name = parts.next().filter(|s| !s.is_empty())?;
    Some(format!("{owner}/{name}"))
}

fn make_row(
    id: String,
    at: DateTime<Utc>,
    title: String,
    detail: String,
    url: Option<String>,
) -> Row {
    let local = at.with_timezone(&Local);
    let ts = local.timestamp();
    (
        local.date_naive(),
        ts,
        TimelineEvent {
            id,
            time: local.format("%H:%M").to_string(),
            timestamp: ts,
            source: TimelineEventSource::Github,
            title,
            detail: Some(detail),
            url: url.and_then(|u| sanitize_event_url(&u)),
        },
    )
}

/// `item["user"]["login"]` is `user` (GitHub logins are case-insensitive).
fn is_user(item: &Value, user: &str) -> bool {
    j_str(item, &["user", "login"]).is_some_and(|login| login.eq_ignore_ascii_case(user))
}

fn timestamp_in_range(item: &Value, key: &str, range: &Range) -> Option<DateTime<Utc>> {
    let at = parse_github_datetime(&j_str(item, &[key])?).ok()?;
    range.contains(at).then_some(at)
}

/// A PR or issue the user opened (a `/search/issues` hit for `author:`).
fn map_opened(item: &Value, range: &Range) -> Option<Row> {
    let issue = issue_ref(item)?;
    let at = timestamp_in_range(item, "created_at", range)?;
    let db_id = j_u64(item, &["id"])?;
    let (kind, noun) = if issue.is_pr {
        ("pr", "Pull request")
    } else {
        ("issue", "Issue")
    };
    let title = format_with_optional_subject(
        format!("{noun} #{} opened", issue.number),
        issue.subject(),
    );
    Some(make_row(
        format!("github:{kind}:{db_id}:opened"),
        at,
        title,
        issue.repo.clone(),
        Some(issue.html_url()),
    ))
}

/// One comment from `/repos/{repo}/issues/{n}/comments`. PR conversation
/// comments arrive here too; the URL uses `pull/` for those so the anchor
/// lands on the PR's conversation tab.
fn map_issue_comment(comment: &Value, issue: &IssueRef, user: &str, range: &Range) -> Option<Row> {
    if !is_user(comment, user) {
        return None;
    }
    let at = timestamp_in_range(comment, "created_at", range)?;
    let comment_id = j_u64(comment, &["id"])?;
    let title = format_with_optional_subject(
        format!("Comment on #{}", issue.number),
        issue.subject(),
    );
    Some(make_row(
        format!("github:comment:{comment_id}"),
        at,
        title,
        issue.repo.clone(),
        Some(format!("{}#issuecomment-{comment_id}", issue.html_url())),
    ))
}

/// One review from `/repos/{repo}/pulls/{n}/reviews`. Pending (unsubmitted)
/// reviews are skipped; state is lower-cased to match the Events API wording.
fn map_pr_review(review: &Value, pr: &IssueRef, user: &str, range: &Range) -> Option<Row> {
    if !is_user(review, user) {
        return None;
    }
    let state = j_str(review, &["state"])
        .unwrap_or_else(|| "reviewed".to_string())
        .to_ascii_lowercase();
    if state == "pending" {
        return None;
    }
    let at = timestamp_in_range(review, "submitted_at", range)?;
    let review_id = j_u64(review, &["id"])?;
    let title = format_with_optional_subject(
        format!("PR review ({state}) on #{}", pr.number),
        pr.subject(),
    );
    Some(make_row(
        format!("github:review:{review_id}"),
        at,
        title,
        pr.repo.clone(),
        Some(format!("{}#pullrequestreview-{review_id}", pr.html_url())),
    ))
}

/// One inline comment from `/repos/{repo}/pulls/{n}/comments`.
fn map_pr_review_comment(
    comment: &Value,
    pr: &IssueRef,
    user: &str,
    range: &Range,
) -> Option<Row> {
    if !is_user(comment, user) {
        return None;
    }
    let at = timestamp_in_range(comment, "created_at", range)?;
    let comment_id = j_u64(comment, &["id"])?;
    let title = format_with_optional_subject(
        format!("PR review comment on #{}", pr.number),
        pr.subject(),
    );
    Some(make_row(
        format!("github:review-comment:{comment_id}"),
        at,
        title,
        pr.repo.clone(),
        Some(format!("{}#discussion_r{comment_id}", pr.html_url())),
    ))
}

/// One `/search/commits` hit (default branch only, placed by author date).
/// Commits already covered by a push event are skipped.
fn map_commit(item: &Value, range: &Range, pushed_shas: &HashSet<String>) -> Option<Row> {
    let sha = j_str(item, &["sha"])?;
    if pushed_shas.contains(&sha) {
        return None;
    }
    let at = parse_github_datetime(&j_str(item, &["commit", "author", "date"])?).ok()?;
    if !range.contains(at) {
        return None;
    }
    let repo = j_str(item, &["repository", "full_name"])?;
    let subject = j_str(item, &["commit", "message"])
        .and_then(|m| m.lines().next().map(str::to_string))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Commit".to_string());
    let short = &sha[..7.min(sha.len())];
    let url = j_str(item, &["html_url"])
        .unwrap_or_else(|| format!("https://github.com/{repo}/commit/{sha}"));
    Some(make_row(
        format!("github:commit:{sha}"),
        at,
        subject,
        format!("{repo} — {short}"),
        Some(url),
    ))
}

struct MappedGithub {
    title: String,
    detail: String,
    url: Option<String>,
}

/// Shas of the commits carried by a push event (at most 20 per event).
fn push_event_shas(ev: &GhEvent) -> Vec<String> {
    ev.payload
        .get("commits")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(|c| j_str(c, &["sha"])).collect())
        .unwrap_or_default()
}

fn map_push_event(ev: &GhEvent) -> Option<MappedGithub> {
    if ev.event_type != "PushEvent" {
        return None;
    }
    let repo = ev.repo.name.clone();
    // The `/users/{username}/events` endpoint strips `html_url` from nested
    // payload objects (only api.github.com URLs remain), so the URL is
    // reconstructed from the repo slug plus the push's before/head shas.
    let branch = j_str(&ev.payload, &["ref"])
        .map(|r| r.trim_start_matches("refs/heads/").to_string())
        .unwrap_or_else(|| "unknown branch".to_string());
    let size = ev.payload.get("size").and_then(|v| v.as_u64()).unwrap_or(1);
    let before = j_str(&ev.payload, &["before"]).unwrap_or_default();
    let head = j_str(&ev.payload, &["head"]).unwrap_or_default();
    let url = if !before.is_empty() && !head.is_empty() {
        Some(format!("https://github.com/{repo}/compare/{before}...{head}"))
    } else {
        Some(format!("https://github.com/{repo}/tree/{branch}"))
    };
    let commit_word = if size == 1 { "commit" } else { "commits" };
    let messages: Vec<String> = ev
        .payload
        .get("commits")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| j_str(c, &["message"]))
                .map(|m| m.lines().next().unwrap_or("").to_string())
                .filter(|m| !m.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let detail = if messages.is_empty() {
        repo
    } else {
        format!("{repo} — {}", messages.join("; "))
    };
    Some(MappedGithub {
        title: format!("Pushed {size} {commit_word} to {branch}"),
        detail,
        url,
    })
}

fn format_with_optional_subject(head: String, subject: Option<String>) -> String {
    match subject {
        Some(s) if !s.trim().is_empty() => format!("{head}: {s}"),
        _ => head,
    }
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

pub(super) fn test_connection(state: &State<'_, AppState>) -> Result<(), String> {
    let Some(settings) = get_settings_github(state.clone()) else {
        return Err("GitHub is not configured".into());
    };
    if settings.username.is_empty() || settings.token.is_empty() {
        return Err("Username and Personal Access Token are required".into());
    }
    let auth = format!("Bearer {}", settings.token);
    match ureq::get(&format!("{API}/user"))
        .header("Authorization", &auth)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", USER_AGENT)
        .call()
    {
        Ok(_) => Ok(()),
        Err(ureq::Error::StatusCode(status)) => Err(format!(
            "GitHub API returned HTTP {status} — check your username and token"
        )),
        Err(e) => Err(format!("GitHub request failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn utc(s: &str) -> DateTime<Utc> {
        parse_github_datetime(s).unwrap()
    }

    /// 2026-09-01T00:00Z ..< 2026-09-08T00:00Z
    fn week() -> Range {
        Range {
            since: utc("2026-09-01T00:00:00Z"),
            until: utc("2026-09-08T00:00:00Z"),
        }
    }

    fn pr_ref() -> IssueRef {
        IssueRef {
            repo: "reload/bupl".into(),
            number: 1535,
            title: "Update Drupal".into(),
            is_pr: true,
        }
    }

    fn issue_ref_fixture() -> IssueRef {
        IssueRef {
            repo: "reload/bupl".into(),
            number: 7,
            title: "Broken build".into(),
            is_pr: false,
        }
    }

    // --- pure helpers ---

    #[test]
    fn parse_github_datetime_valid() {
        assert!(parse_github_datetime("2024-01-15T10:30:00Z").is_ok());
        assert!(parse_github_datetime("2024-01-15T10:30:00+02:00").is_ok());
        // /search/commits author dates carry milliseconds and an offset.
        assert!(parse_github_datetime("2026-09-17T10:58:36.000+02:00").is_ok());
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
    fn validate_username_accepts_github_logins() {
        assert!(validate_username("rasben").is_ok());
        assert!(validate_username("octo-cat42").is_ok());
    }

    #[test]
    fn validate_username_rejects_qualifier_injection() {
        assert!(validate_username("").is_err());
        assert!(validate_username("me OR commenter:you").is_err());
        assert!(validate_username("a:b").is_err());
    }

    #[test]
    fn days_between_inclusive() {
        let a = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let b = NaiveDate::from_ymd_opt(2026, 9, 3).unwrap();
        assert_eq!(days_between(a, b).len(), 3);
        assert_eq!(days_between(a, a), vec![a]);
    }

    // --- range / query building ---

    #[test]
    fn range_contains_is_half_open() {
        let r = week();
        assert!(r.contains(utc("2026-09-01T00:00:00Z")));
        assert!(r.contains(utc("2026-09-07T23:59:59Z")));
        assert!(!r.contains(utc("2026-09-08T00:00:00Z")));
        assert!(!r.contains(utc("2026-08-31T23:59:59Z")));
    }

    #[test]
    fn created_qualifier_is_inclusive_iso_with_offset() {
        assert_eq!(
            week().created_qualifier(),
            "2026-09-01T00:00:00+00:00..2026-09-07T23:59:59+00:00"
        );
    }

    #[test]
    fn since_qualifier_and_param() {
        assert_eq!(week().since_qualifier(), ">=2026-09-01T00:00:00+00:00");
        assert_eq!(week().since_param(), "2026-09-01T00:00:00Z");
    }

    #[test]
    fn opened_query_variants() {
        let r = week();
        assert_eq!(opened_query("me", false, false, &r), None);
        assert_eq!(
            opened_query("me", true, false, &r).unwrap(),
            "is:pr author:me created:2026-09-01T00:00:00+00:00..2026-09-07T23:59:59+00:00"
        );
        assert!(opened_query("me", false, true, &r)
            .unwrap()
            .starts_with("is:issue author:me created:"));
        assert!(opened_query("me", true, true, &r)
            .unwrap()
            .starts_with("author:me created:"));
    }

    #[test]
    fn candidate_query_variants() {
        let r = week();
        assert_eq!(candidate_query("me", false, false, &r), None);
        assert_eq!(
            candidate_query("me", true, false, &r).unwrap(),
            "commenter:me updated:>=2026-09-01T00:00:00+00:00"
        );
        assert_eq!(
            candidate_query("me", false, true, &r).unwrap(),
            "is:pr reviewed-by:me updated:>=2026-09-01T00:00:00+00:00"
        );
        assert_eq!(
            candidate_query("me", true, true, &r).unwrap(),
            "(commenter:me OR reviewed-by:me) updated:>=2026-09-01T00:00:00+00:00"
        );
    }

    #[test]
    fn search_url_adds_advanced_search_for_issues_only() {
        let issues = search_url("issues", "author:me is:pr", 2);
        assert!(issues.starts_with("https://api.github.com/search/issues?q=author%3Ame%20is%3Apr"));
        assert!(issues.contains("&per_page=100&page=2"));
        assert!(issues.ends_with("&advanced_search=true"));

        let commits = search_url("commits", "author:me", 1);
        assert!(commits.starts_with("https://api.github.com/search/commits?q="));
        assert!(!commits.contains("advanced_search"));
    }

    #[test]
    fn push_horizon_reached_since_means_complete_range() {
        let r = week();
        let now = utc("2026-09-10T00:00:00Z");
        let h = push_horizon(true, Some(utc("2026-09-03T00:00:00Z")), &r, now);
        assert_eq!(h, r.since);
    }

    #[test]
    fn push_horizon_ran_dry_uses_oldest_event() {
        let r = week();
        let now = utc("2026-09-10T00:00:00Z");
        let oldest = utc("2026-09-04T12:00:00Z");
        assert_eq!(push_horizon(false, Some(oldest), &r, now), oldest);
    }

    #[test]
    fn push_horizon_ran_dry_without_events_is_the_api_floor() {
        // Range older than 90 days: nothing from the Events API is complete,
        // but the floor is clamped to the range start when the range is newer.
        let old = Range {
            since: utc("2026-01-01T00:00:00Z"),
            until: utc("2026-01-08T00:00:00Z"),
        };
        let now = utc("2026-09-10T00:00:00Z");
        assert_eq!(
            push_horizon(false, None, &old, now),
            now - Duration::days(EVENTS_API_MAX_AGE_DAYS)
        );
        let recent = week();
        assert_eq!(push_horizon(false, None, &recent, now), recent.since);
    }

    #[test]
    fn push_horizon_never_before_90_day_floor() {
        let old = Range {
            since: utc("2026-01-01T00:00:00Z"),
            until: utc("2026-01-08T00:00:00Z"),
        };
        let now = utc("2026-09-10T00:00:00Z");
        // Even a "reached since" claim can't push the horizon past the floor.
        assert_eq!(
            push_horizon(true, None, &old, now),
            now - Duration::days(EVENTS_API_MAX_AGE_DAYS)
        );
    }

    // --- search hit parsing ---

    #[test]
    fn issue_ref_from_search_item() {
        let item = json!({
            "number": 1535,
            "title": "Update Drupal",
            "repository_url": "https://api.github.com/repos/reload/bupl",
            "html_url": "https://github.com/reload/bupl/pull/1535",
            "pull_request": {"url": "https://api.github.com/repos/reload/bupl/pulls/1535"}
        });
        assert_eq!(issue_ref(&item), Some(pr_ref()));
    }

    #[test]
    fn issue_ref_falls_back_to_html_url_and_detects_issues() {
        let item = json!({
            "number": 7,
            "title": "Broken build",
            "html_url": "https://github.com/reload/bupl/issues/7"
        });
        assert_eq!(issue_ref(&item), Some(issue_ref_fixture()));
    }

    #[test]
    fn issue_ref_requires_number() {
        let item = json!({"repository_url": "https://api.github.com/repos/a/b"});
        assert_eq!(issue_ref(&item), None);
    }

    #[test]
    fn repo_from_urls() {
        assert_eq!(
            repo_from_repository_url("https://api.github.com/repos/reload/bupl"),
            Some("reload/bupl".into())
        );
        assert_eq!(repo_from_repository_url("https://example.com/x"), None);
        assert_eq!(
            repo_from_html_url("https://github.com/reload/bupl/pull/1#issuecomment-2"),
            Some("reload/bupl".into())
        );
        assert_eq!(repo_from_html_url("https://github.com/only-owner"), None);
    }

    // --- mapping ---

    #[test]
    fn map_opened_pull_request() {
        let item = json!({
            "id": 5488697526u64,
            "number": 1535,
            "title": "Update Drupal",
            "repository_url": "https://api.github.com/repos/reload/bupl",
            "pull_request": {},
            "created_at": "2026-09-03T14:24:39Z"
        });
        let (_, ts, ev) = map_opened(&item, &week()).unwrap();
        assert_eq!(ts, utc("2026-09-03T14:24:39Z").timestamp());
        assert_eq!(ev.timestamp, ts);
        assert_eq!(ev.id, "github:pr:5488697526:opened");
        assert_eq!(ev.title, "Pull request #1535 opened: Update Drupal");
        assert_eq!(ev.detail.as_deref(), Some("reload/bupl"));
        assert_eq!(
            ev.url.as_deref(),
            Some("https://github.com/reload/bupl/pull/1535")
        );
        assert!(matches!(ev.source, TimelineEventSource::Github));
    }

    #[test]
    fn map_opened_issue_without_title() {
        let item = json!({
            "id": 99,
            "number": 7,
            "title": "",
            "repository_url": "https://api.github.com/repos/reload/bupl",
            "created_at": "2026-09-03T14:24:39Z"
        });
        let (_, _, ev) = map_opened(&item, &week()).unwrap();
        assert_eq!(ev.id, "github:issue:99:opened");
        assert_eq!(ev.title, "Issue #7 opened");
        assert_eq!(
            ev.url.as_deref(),
            Some("https://github.com/reload/bupl/issues/7")
        );
    }

    #[test]
    fn map_opened_outside_range_is_dropped() {
        let item = json!({
            "id": 1,
            "number": 1,
            "repository_url": "https://api.github.com/repos/a/b",
            "created_at": "2026-09-08T00:00:00Z"
        });
        assert!(map_opened(&item, &week()).is_none());
    }

    #[test]
    fn map_issue_comment_on_pr_uses_pull_path() {
        let c = json!({
            "id": 3344,
            "user": {"login": "RasBen"},
            "created_at": "2026-09-02T09:00:00Z"
        });
        let (_, _, ev) = map_issue_comment(&c, &pr_ref(), "rasben", &week()).unwrap();
        assert_eq!(ev.id, "github:comment:3344");
        assert_eq!(ev.title, "Comment on #1535: Update Drupal");
        assert_eq!(
            ev.url.as_deref(),
            Some("https://github.com/reload/bupl/pull/1535#issuecomment-3344")
        );
    }

    #[test]
    fn map_issue_comment_on_issue_uses_issues_path() {
        let c = json!({
            "id": 5,
            "user": {"login": "rasben"},
            "created_at": "2026-09-02T09:00:00Z"
        });
        let (_, _, ev) = map_issue_comment(&c, &issue_ref_fixture(), "rasben", &week()).unwrap();
        assert_eq!(
            ev.url.as_deref(),
            Some("https://github.com/reload/bupl/issues/7#issuecomment-5")
        );
    }

    #[test]
    fn map_issue_comment_filters_other_users_and_range() {
        let other = json!({
            "id": 1,
            "user": {"login": "someone-else"},
            "created_at": "2026-09-02T09:00:00Z"
        });
        assert!(map_issue_comment(&other, &pr_ref(), "rasben", &week()).is_none());
        let too_old = json!({
            "id": 2,
            "user": {"login": "rasben"},
            "created_at": "2026-08-30T09:00:00Z"
        });
        assert!(map_issue_comment(&too_old, &pr_ref(), "rasben", &week()).is_none());
    }

    #[test]
    fn map_pr_review_lowercases_state_and_anchors() {
        let r = json!({
            "id": 777,
            "state": "CHANGES_REQUESTED",
            "user": {"login": "rasben"},
            "submitted_at": "2026-09-05T10:00:00Z"
        });
        let (_, _, ev) = map_pr_review(&r, &pr_ref(), "rasben", &week()).unwrap();
        assert_eq!(ev.id, "github:review:777");
        assert_eq!(
            ev.title,
            "PR review (changes_requested) on #1535: Update Drupal"
        );
        assert_eq!(
            ev.url.as_deref(),
            Some("https://github.com/reload/bupl/pull/1535#pullrequestreview-777")
        );
    }

    #[test]
    fn map_pr_review_skips_pending() {
        let r = json!({
            "id": 1,
            "state": "PENDING",
            "user": {"login": "rasben"},
            "submitted_at": null
        });
        assert!(map_pr_review(&r, &pr_ref(), "rasben", &week()).is_none());
    }

    #[test]
    fn map_pr_review_comment_anchor() {
        let c = json!({
            "id": 4242,
            "user": {"login": "rasben"},
            "created_at": "2026-09-05T10:00:00Z"
        });
        let (_, _, ev) = map_pr_review_comment(&c, &pr_ref(), "rasben", &week()).unwrap();
        assert_eq!(ev.id, "github:review-comment:4242");
        assert_eq!(ev.title, "PR review comment on #1535: Update Drupal");
        assert_eq!(
            ev.url.as_deref(),
            Some("https://github.com/reload/bupl/pull/1535#discussion_r4242")
        );
    }

    #[test]
    fn map_commit_from_search() {
        let item = json!({
            "sha": "579d4efa4b2cb6e72bd966774fa7ee3ff83392d4",
            "html_url": "https://github.com/reload/joyspeed/commit/579d4efa4b2cb6e72bd966774fa7ee3ff83392d4",
            "commit": {
                "author": {"date": "2026-09-03T10:58:36.000+02:00"},
                "message": "Merge pull request #576\n\nbody text"
            },
            "repository": {"full_name": "reload/joyspeed"}
        });
        let (_, ts, ev) = map_commit(&item, &week(), &HashSet::new()).unwrap();
        assert_eq!(ts, utc("2026-09-03T08:58:36Z").timestamp());
        assert_eq!(ev.id, "github:commit:579d4efa4b2cb6e72bd966774fa7ee3ff83392d4");
        assert_eq!(ev.title, "Merge pull request #576");
        assert_eq!(ev.detail.as_deref(), Some("reload/joyspeed — 579d4ef"));
        assert_eq!(
            ev.url.as_deref(),
            Some("https://github.com/reload/joyspeed/commit/579d4efa4b2cb6e72bd966774fa7ee3ff83392d4")
        );
    }

    #[test]
    fn map_commit_skips_shas_already_pushed() {
        let item = json!({
            "sha": "abc",
            "commit": {"author": {"date": "2026-09-03T10:00:00Z"}, "message": "x"},
            "repository": {"full_name": "a/b"}
        });
        let pushed: HashSet<String> = ["abc".to_string()].into_iter().collect();
        assert!(map_commit(&item, &week(), &pushed).is_none());
        assert!(map_commit(&item, &week(), &HashSet::new()).is_some());
    }

    fn push_event() -> GhEvent {
        serde_json::from_value(json!({
            "id": "123456",
            "type": "PushEvent",
            "repo": {"name": "reload/bupl"},
            "created_at": "2026-09-03T10:00:00Z",
            "payload": {
                "ref": "refs/heads/feature/x",
                "size": 2,
                "before": "aaa111",
                "head": "bbb222",
                "commits": [
                    {"sha": "aaa222", "message": "First\n\nbody"},
                    {"sha": "bbb222", "message": "Second"}
                ]
            }
        }))
        .unwrap()
    }

    #[test]
    fn map_push_event_shape() {
        let m = map_push_event(&push_event()).unwrap();
        assert_eq!(m.title, "Pushed 2 commits to feature/x");
        assert_eq!(m.detail, "reload/bupl — First; Second");
        assert_eq!(
            m.url.as_deref(),
            Some("https://github.com/reload/bupl/compare/aaa111...bbb222")
        );
    }

    #[test]
    fn map_push_event_ignores_other_types() {
        let mut ev = push_event();
        ev.event_type = "WatchEvent".into();
        assert!(map_push_event(&ev).is_none());
    }

    #[test]
    fn push_event_shas_collects_commit_shas() {
        assert_eq!(push_event_shas(&push_event()), vec!["aaa222", "bbb222"]);
    }

    #[test]
    fn make_row_sanitizes_url() {
        let (_, _, ev) = make_row(
            "github:x".into(),
            utc("2026-09-03T10:00:00Z"),
            "t".into(),
            "d".into(),
            Some("javascript:alert(1)".into()),
        );
        assert_eq!(ev.url, None);
    }

    #[test]
    fn parallel_fetch_concatenates_and_propagates_errors() {
        let ok = parallel_fetch(vec![1u64, 2, 3], |n| {
            Ok(vec![make_row(
                format!("github:{n}"),
                utc("2026-09-03T10:00:00Z"),
                "t".into(),
                "d".into(),
                None,
            )])
        })
        .unwrap();
        let mut ids: Vec<String> = ok.into_iter().map(|(_, _, ev)| ev.id).collect();
        ids.sort();
        assert_eq!(ids, vec!["github:1", "github:2", "github:3"]);

        let err = parallel_fetch(vec![1u64, 2], |n| {
            if n == 2 {
                Err("boom".to_string())
            } else {
                Ok(Vec::new())
            }
        });
        assert_eq!(err.err(), Some("boom".to_string()));

        let empty: Result<Vec<Row>, String> = parallel_fetch(Vec::<u64>::new(), |_| Ok(Vec::new()));
        assert_eq!(empty.unwrap().len(), 0);
    }

    // --- integration (skipped when secrets absent) ---

    fn test_credentials() -> Option<(String, String)> {
        let token = std::env::var("RECALL_TEST_GITHUB_TOKEN").ok()?;
        let username = std::env::var("RECALL_TEST_GITHUB_USERNAME").ok()?;
        Some((token, username))
    }

    #[test]
    fn github_api_returns_events_with_valid_credentials() {
        let Some((token, username)) = test_credentials() else {
            return;
        };
        let client = GhClient::new(&token);
        let result = rest_api_user_events(&client, &username, 0);
        assert!(
            result.is_ok(),
            "GitHub API call failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn github_search_api_accepts_our_queries() {
        let Some((token, username)) = test_credentials() else {
            return;
        };
        let client = GhClient::new(&token);
        let now = Utc::now();
        let range = Range {
            since: now - Duration::days(7),
            until: now,
        };
        let opened = opened_query(&username, true, true, &range).unwrap();
        let result = search(&client, "issues", &opened);
        assert!(result.is_ok(), "search/issues (opened) failed: {:?}", result.err());

        let candidates = candidate_query(&username, true, true, &range).unwrap();
        let result = search(&client, "issues", &candidates);
        assert!(result.is_ok(), "search/issues (OR query) failed: {:?}", result.err());

        // Exercise the follow-up list endpoints on the first real candidate so
        // the `since=` parameter and pagination are covered end to end.
        if let Some(first) = result.unwrap().items.iter().find_map(issue_ref) {
            let comments = fetch_issue_comments(&client, &first, &username, &range);
            assert!(comments.is_ok(), "issue comments failed: {:?}", comments.err());
            if first.is_pr {
                let reviews = fetch_pr_reviews(&client, &first, &username, &range);
                assert!(reviews.is_ok(), "PR reviews failed: {:?}", reviews.err());
                let review_comments = fetch_pr_review_comments(&client, &first, &username, &range);
                assert!(
                    review_comments.is_ok(),
                    "PR review comments failed: {:?}",
                    review_comments.err()
                );
            }
        }

        let commits = format!(
            "author:{username} author-date:{}",
            range.created_qualifier()
        );
        let result = search(&client, "commits", &commits);
        assert!(result.is_ok(), "search/commits failed: {:?}", result.err());
    }
}
