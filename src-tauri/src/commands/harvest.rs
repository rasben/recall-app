//! Read-only Harvest API v2 client: who am I, and what have I already logged
//! on a given day. Harvest entries change after the fact (the user edits them
//! in Harvest itself), so nothing here goes through `timeline_day_cache` —
//! every call fetches live.
//!
//! API reference: https://help.getharvest.com/api-v2/

use crate::commands::settings_harvest::get_settings_harvest;
use crate::state::AppState;
use crate::timeline::sanitize_event_url;
use chrono::NaiveDate;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Mutex;
use tauri::State;

const API_BASE: &str = "https://api.harvestapp.com/v2";
/// Harvest rejects requests without a User-Agent (400) and asks that it name
/// the app plus a contact URL or email.
const USER_AGENT: &str = "recall-app (https://github.com/rasben/recall-app)";
/// A single day never has thousands of entries; this only guards against a
/// runaway pagination loop.
const MAX_PAGES: u32 = 10;

/// One Harvest time entry as shown in the timeline's Harvest strip.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct HarvestTimeEntry {
    pub id: i64,
    /// Decimal hours as tracked (Harvest also has `rounded_hours`; we show
    /// what the user actually entered).
    pub hours: f64,
    pub notes: Option<String>,
    pub project: String,
    pub client: String,
    pub task: String,
    pub is_running: bool,
    /// The Harvest web page for this user's day, when the account's web
    /// domain could be resolved. Harvest has no documented per-entry deep link.
    pub url: Option<String>,
}

// --- HTTP ---------------------------------------------------------------

/// `GET {API_BASE}{path}` with the three headers Harvest requires. Returns the
/// HTTP status and body; an HTTP error status is returned as `Ok((status, body))`
/// so callers can produce a message, transport failures as `Err`.
pub(crate) fn harvest_get(
    label: &str,
    path: &str,
    token: &str,
    account_id: &str,
) -> Result<(u16, String), String> {
    let url = format!("{API_BASE}{path}");
    let auth = format!("Bearer {token}");
    let mut resp = match ureq::get(&url)
        .header("Authorization", &auth)
        .header("Harvest-Account-Id", account_id)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .call()
    {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(status)) => return Ok((status, String::new())),
        Err(e) => return Err(format!("Harvest HTTP ({label}): {e}")),
    };
    let status = resp.status().as_u16();
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("read body ({label}): {e}"))?;
    Ok((status, body))
}

/// Human-readable error for a non-2xx Harvest response.
fn http_error(label: &str, status: u16, body: &str) -> String {
    let description = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.get("error_description")
                .or_else(|| v.get("message"))
                .and_then(|d| d.as_str())
                .map(str::to_string)
        });
    match (status, description) {
        (401 | 403, Some(d)) => format!("Harvest {label}: HTTP {status} — {d}"),
        (401 | 403, None) => {
            format!("Harvest {label}: HTTP {status} — check your access token and account ID")
        }
        (_, Some(d)) => format!("Harvest {label}: HTTP {status} — {d}"),
        (_, None) => format!("Harvest {label}: HTTP {status}"),
    }
}

// --- Identity (who am I, which web domain) ------------------------------

#[derive(Debug, Deserialize)]
pub(crate) struct HarvestUser {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
struct HarvestCompany {
    #[serde(default)]
    base_uri: Option<String>,
}

/// `GET /v2/users/me`. Used by the settings "test connection" button and to
/// learn the user id that scopes the time-entry query.
pub(crate) fn fetch_current_user(token: &str, account_id: &str) -> Result<HarvestUser, String> {
    let (status, body) = harvest_get("users/me", "/users/me", token, account_id)?;
    if status >= 400 {
        return Err(http_error("users/me", status, &body));
    }
    serde_json::from_str(&body).map_err(|e| format!("Harvest users/me JSON: {e}"))
}

/// `GET /v2/company` → the account's web `base_uri`. Best-effort: the strip
/// works without links, so a failure here degrades to `None`.
fn fetch_company_base_uri(token: &str, account_id: &str) -> Option<String> {
    let (status, body) = harvest_get("company", "/company", token, account_id).ok()?;
    if status >= 400 {
        return None;
    }
    let company: HarvestCompany = serde_json::from_str(&body).ok()?;
    company.base_uri
}

#[derive(Clone)]
struct Identity {
    token: String,
    account_id: String,
    user_id: i64,
    base_uri: Option<String>,
}

/// The identity behind the current credentials, so a day switch costs one
/// request instead of three. Keyed on the credentials: change the token and
/// the next call refetches.
static IDENTITY: Lazy<Mutex<Option<Identity>>> = Lazy::new(|| Mutex::new(None));

fn identity(token: &str, account_id: &str) -> Result<Identity, String> {
    if let Ok(guard) = IDENTITY.lock() {
        if let Some(id) = guard.as_ref() {
            if id.token == token && id.account_id == account_id {
                return Ok(id.clone());
            }
        }
    }
    let user = fetch_current_user(token, account_id)?;
    let ident = Identity {
        token: token.to_string(),
        account_id: account_id.to_string(),
        user_id: user.id,
        base_uri: fetch_company_base_uri(token, account_id),
    };
    if let Ok(mut guard) = IDENTITY.lock() {
        *guard = Some(ident.clone());
    }
    Ok(ident)
}

fn forget_identity() {
    if let Ok(mut guard) = IDENTITY.lock() {
        *guard = None;
    }
}

// --- Time entries -------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TimeEntriesPage {
    #[serde(default)]
    time_entries: Vec<RawTimeEntry>,
    #[serde(default)]
    next_page: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RawTimeEntry {
    id: i64,
    spent_date: String,
    #[serde(default)]
    hours: f64,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    project: Option<Named>,
    #[serde(default)]
    client: Option<Named>,
    #[serde(default)]
    task: Option<Named>,
    #[serde(default)]
    is_running: bool,
}

#[derive(Debug, Deserialize)]
struct Named {
    #[serde(default)]
    name: Option<String>,
}

fn parse_time_entries_page(body: &str) -> Result<TimeEntriesPage, String> {
    serde_json::from_str(body).map_err(|e| format!("Harvest time_entries JSON: {e}"))
}

fn name_of(named: &Option<Named>) -> String {
    named
        .as_ref()
        .and_then(|n| n.name.clone())
        .unwrap_or_default()
}

fn to_entry(raw: RawTimeEntry, url: Option<String>) -> HarvestTimeEntry {
    HarvestTimeEntry {
        id: raw.id,
        hours: raw.hours,
        notes: raw
            .notes
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty()),
        project: name_of(&raw.project),
        client: name_of(&raw.client),
        task: name_of(&raw.task),
        is_running: raw.is_running,
        url,
    }
}

/// The Harvest web "day" view for `user_id` on `day`, e.g.
/// `https://acme.harvestapp.com/time/day/2026/09/21/1234567`. `None` when
/// `base_uri` is not an http(s) URL.
fn harvest_day_url(base_uri: &str, day: NaiveDate, user_id: i64) -> Option<String> {
    let base = base_uri.trim().trim_end_matches('/');
    let url = format!("{base}/time/day/{}/{user_id}", day.format("%Y/%m/%d"));
    sanitize_event_url(&url)
}

/// All of `user_id`'s entries with `spent_date == day`, following pagination.
fn fetch_time_entries_for_day(
    token: &str,
    account_id: &str,
    user_id: i64,
    day: &str,
) -> Result<Vec<RawTimeEntry>, String> {
    let mut all = Vec::new();
    let mut page: u32 = 1;
    loop {
        let path = format!(
            "/time_entries?user_id={user_id}&from={day}&to={day}&per_page=100&page={page}"
        );
        let (status, body) = harvest_get("time_entries", &path, token, account_id)?;
        if status == 401 || status == 403 {
            // Credentials went bad since we cached the identity.
            forget_identity();
        }
        if status >= 400 {
            return Err(http_error("time_entries", status, &body));
        }
        let parsed = parse_time_entries_page(&body)?;
        all.extend(parsed.time_entries);
        match parsed.next_page {
            Some(next) if next > page && page < MAX_PAGES => page = next,
            _ => break,
        }
    }
    Ok(all)
}

/// The authenticated user's Harvest time entries for `day` (`YYYY-MM-DD`),
/// oldest first. Returns an empty list when Harvest is disabled or has no
/// credentials, so the frontend can call it unconditionally.
#[tauri::command]
#[specta::specta]
pub async fn get_harvest_entries_for_day(
    state: State<'_, AppState>,
    day: String,
) -> Result<Vec<HarvestTimeEntry>, String> {
    tokio::task::block_in_place(|| {
        let day_naive = NaiveDate::parse_from_str(&day, "%Y-%m-%d")
            .map_err(|_| format!("Invalid date (expected YYYY-MM-DD): {day}"))?;
        let Some(settings) = get_settings_harvest(state.clone()) else {
            return Ok(Vec::new());
        };
        let Some((token, account_id)) = settings.credentials() else {
            return Ok(Vec::new());
        };

        let ident = identity(token, account_id)?;
        let url = ident
            .base_uri
            .as_deref()
            .and_then(|base| harvest_day_url(base, day_naive, ident.user_id));

        let day_str = day_naive.format("%Y-%m-%d").to_string();
        let mut raws = fetch_time_entries_for_day(token, account_id, ident.user_id, &day_str)?;
        // `from`/`to` filter on spent_date already; keep the guard in case the
        // API ever widens the window.
        raws.retain(|r| r.spent_date == day_str);
        raws.sort_by_key(|r| r.id);
        Ok(raws.into_iter().map(|r| to_entry(r, url.clone())).collect())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed from the documented example response for GET /v2/time_entries.
    const EXAMPLE_PAGE: &str = r#"{
      "time_entries": [
        {
          "id": 636709355,
          "spent_date": "2017-03-02",
          "user": {"id": 1782959, "name": "Kim Allen"},
          "client": {"id": 5735774, "name": "ABC Corp"},
          "project": {"id": 14307913, "name": "Marketing Website"},
          "task": {"id": 8083365, "name": "Graphic Design"},
          "hours": 2.11,
          "rounded_hours": 2.25,
          "notes": "Adding CSS styling",
          "is_locked": true,
          "is_running": false,
          "approval_status": "approved",
          "is_billed": false,
          "created_at": "2017-06-27T15:50:15Z",
          "updated_at": "2017-06-27T16:47:14Z"
        },
        {
          "id": 636708723,
          "spent_date": "2017-03-02",
          "user": {"id": 1782959, "name": "Kim Allen"},
          "client": {"id": 5735776, "name": "123 Industries"},
          "project": {"id": 14308069, "name": "Online Store - Phase 1"},
          "task": {"id": 8083366, "name": "Programming"},
          "hours": 1.35,
          "notes": null,
          "is_running": true,
          "timer_started_at": "2017-03-02T14:00:00Z"
        }
      ],
      "per_page": 2000,
      "total_pages": 1,
      "total_entries": 2,
      "next_page": null,
      "previous_page": null,
      "page": 1
    }"#;

    // --- pure helpers ---

    #[test]
    fn parse_time_entries_page_reads_documented_example() {
        let page = parse_time_entries_page(EXAMPLE_PAGE).unwrap();
        assert_eq!(page.time_entries.len(), 2);
        assert_eq!(page.next_page, None);
        let first = &page.time_entries[0];
        assert_eq!(first.id, 636709355);
        assert_eq!(first.spent_date, "2017-03-02");
        assert!((first.hours - 2.11).abs() < f64::EPSILON);
        assert_eq!(first.notes.as_deref(), Some("Adding CSS styling"));
        assert!(!first.is_running);
        assert!(page.time_entries[1].is_running);
        assert_eq!(page.time_entries[1].notes, None);
    }

    #[test]
    fn parse_time_entries_page_reads_next_page() {
        let body = r#"{"time_entries": [], "next_page": 2, "page": 1}"#;
        let page = parse_time_entries_page(body).unwrap();
        assert!(page.time_entries.is_empty());
        assert_eq!(page.next_page, Some(2));
    }

    #[test]
    fn parse_time_entries_page_tolerates_missing_optional_fields() {
        let body = r#"{"time_entries": [{"id": 1, "spent_date": "2026-09-21"}]}"#;
        let page = parse_time_entries_page(body).unwrap();
        let raw = &page.time_entries[0];
        assert_eq!(raw.hours, 0.0);
        assert!(raw.project.is_none());
        assert!(!raw.is_running);
    }

    #[test]
    fn parse_time_entries_page_rejects_garbage() {
        assert!(parse_time_entries_page("not json").is_err());
    }

    #[test]
    fn to_entry_maps_names_and_trims_notes() {
        let page = parse_time_entries_page(EXAMPLE_PAGE).unwrap();
        let mut it = page.time_entries.into_iter();
        let first = to_entry(it.next().unwrap(), Some("https://x.harvestapp.com/d".into()));
        assert_eq!(
            first,
            HarvestTimeEntry {
                id: 636709355,
                hours: 2.11,
                notes: Some("Adding CSS styling".into()),
                project: "Marketing Website".into(),
                client: "ABC Corp".into(),
                task: "Graphic Design".into(),
                is_running: false,
                url: Some("https://x.harvestapp.com/d".into()),
            }
        );
        let second = to_entry(it.next().unwrap(), None);
        assert_eq!(second.notes, None);
        assert!(second.is_running);
        assert_eq!(second.url, None);
    }

    #[test]
    fn to_entry_blank_notes_become_none() {
        let body = r#"{"time_entries": [{"id": 1, "spent_date": "2026-09-21", "notes": "   "}]}"#;
        let raw = parse_time_entries_page(body)
            .unwrap()
            .time_entries
            .pop()
            .unwrap();
        let entry = to_entry(raw, None);
        assert_eq!(entry.notes, None);
        assert_eq!(entry.project, "");
    }

    #[test]
    fn harvest_day_url_builds_day_view() {
        let day = NaiveDate::from_ymd_opt(2026, 9, 3).unwrap();
        assert_eq!(
            harvest_day_url("https://acme.harvestapp.com/", day, 1234567),
            Some("https://acme.harvestapp.com/time/day/2026/09/03/1234567".into())
        );
    }

    #[test]
    fn harvest_day_url_rejects_non_http_base() {
        let day = NaiveDate::from_ymd_opt(2026, 9, 3).unwrap();
        assert_eq!(harvest_day_url("javascript:alert(1)", day, 1), None);
        assert_eq!(harvest_day_url("", day, 1), None);
    }

    #[test]
    fn http_error_uses_harvest_error_description() {
        let body = r#"{"error":"invalid_token","error_description":"The access token provided is expired, revoked, malformed or invalid for other reasons."}"#;
        let msg = http_error("users/me", 401, body);
        assert!(msg.contains("401"));
        assert!(msg.contains("expired, revoked"));
    }

    #[test]
    fn http_error_without_body_hints_at_credentials() {
        let msg = http_error("users/me", 401, "");
        assert!(msg.contains("access token and account ID"));
        let msg = http_error("time_entries", 500, "");
        assert_eq!(msg, "Harvest time_entries: HTTP 500");
    }

    #[test]
    fn credentials_require_enabled_and_both_fields() {
        use crate::commands::settings_harvest::SettingsHarvest;
        let s = SettingsHarvest {
            enabled: true,
            access_token: " tok ".into(),
            account_id: " 42 ".into(),
        };
        assert_eq!(s.credentials(), Some(("tok", "42")));
        let disabled = SettingsHarvest {
            enabled: false,
            ..s
        };
        assert_eq!(disabled.credentials(), None);
        let missing = SettingsHarvest {
            enabled: true,
            access_token: "tok".into(),
            account_id: "".into(),
        };
        assert_eq!(missing.credentials(), None);
    }

    // --- integration (skipped when secrets absent) ---

    #[test]
    fn harvest_users_me_returns_id_with_valid_credentials() {
        let token = match std::env::var("RECALL_TEST_HARVEST_TOKEN") {
            Ok(t) => t,
            Err(_) => return,
        };
        let account_id = match std::env::var("RECALL_TEST_HARVEST_ACCOUNT_ID") {
            Ok(a) => a,
            Err(_) => return,
        };

        let user = fetch_current_user(&token, &account_id);
        assert!(user.is_ok(), "Harvest users/me failed: {:?}", user.err());
        let user = user.unwrap();
        assert!(user.id > 0, "Harvest users/me returned a non-positive id");

        let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
        let entries = fetch_time_entries_for_day(&token, &account_id, user.id, &today);
        assert!(entries.is_ok(), "Harvest time_entries failed: {:?}", entries.err());
        for raw in entries.unwrap() {
            assert_eq!(raw.spent_date, today);
        }
    }
}
