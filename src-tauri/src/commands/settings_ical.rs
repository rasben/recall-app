use crate::commands::settings::{get_val, now, save_val};
use crate::state::AppState;
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use ical::IcalParser;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::{HashMap, HashSet};
use std::io::BufReader;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::State;

pub(crate) const KEY: &str = "settings_ical";

#[derive(Deserialize, Serialize, Type, Default)]
#[specta(export = false)]
pub struct SettingsIcal {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub urls: Vec<String>,
    /// User's email address used to identify their ATTENDEE entry and filter declined events.
    #[serde(default)]
    pub email: Option<String>,
}

pub(crate) fn load_settings_ical(state: &State<'_, AppState>) -> SettingsIcal {
    get_val(state, KEY)
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default()
}

#[tauri::command]
#[specta::specta]
pub fn get_settings_ical(state: State<'_, AppState>) -> Option<SettingsIcal> {
    get_val(&state, KEY).and_then(|j| serde_json::from_str(&j).ok())
}

#[tauri::command]
#[specta::specta]
pub fn set_settings_ical(
    state: State<'_, AppState>,
    settings: SettingsIcal,
) -> Result<(), String> {
    let old = load_settings_ical(&state);
    let urls_changed = old.urls != settings.urls;
    let should_sync = settings.enabled && (urls_changed || !old.enabled);

    let json = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    save_val(&state, KEY, &json)?;

    if urls_changed {
        let conn = state.db.lock().map_err(|_| "DB lock failed".to_string())?;
        conn.execute("DELETE FROM timeline_day_cache", [])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM ical_events", [])
            .map_err(|e| e.to_string())?;
    }

    if should_sync {
        start_sync(&state);
    }

    Ok(())
}

// ── Sync status ────────────────────────────────────────────────────────────────

#[derive(Serialize, Type, Clone)]
pub struct IcalSyncStatus {
    pub syncing: bool,
    pub last_synced_at: Option<f64>,
    pub last_error: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn get_ical_sync_status(state: State<'_, AppState>) -> IcalSyncStatus {
    let syncing = state.ical_syncing.load(Ordering::Relaxed);
    let conn = match state.db.lock() {
        Ok(c) => c,
        Err(_) => {
            return IcalSyncStatus {
                syncing,
                last_synced_at: None,
                last_error: None,
            }
        }
    };
    let meta: Option<(Option<i64>, Option<String>)> = conn
        .query_row(
            "SELECT last_synced_at, last_error FROM ical_sync_meta WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();
    let (last_synced_at, last_error) = meta.unwrap_or((None, None));
    IcalSyncStatus {
        syncing,
        last_synced_at: last_synced_at.map(|t| t as f64),
        last_error,
    }
}

/// Kicks off a background iCal sync. Returns immediately; poll `get_ical_sync_status` for progress.
#[tauri::command]
#[specta::specta]
pub fn trigger_ical_sync(state: State<'_, AppState>) {
    start_sync(&state);
}

pub(crate) fn start_sync(state: &State<'_, AppState>) {
    if state.ical_syncing.swap(true, Ordering::AcqRel) {
        return; // already running
    }

    let settings = load_settings_ical(state);
    let urls: Vec<String> = settings
        .urls
        .into_iter()
        .filter(|u| !u.trim().is_empty())
        .collect();

    if !settings.enabled || urls.is_empty() {
        state.ical_syncing.store(false, Ordering::Release);
        return;
    }

    // The sync opens its own connection so it never contends with the main DB mutex.
    let db_path = state.db_path.clone();
    let syncing = Arc::clone(&state.ical_syncing);
    let email = settings.email.clone();

    tauri::async_runtime::spawn_blocking(move || {
        run_sync(db_path, syncing, urls, email);
    });
}

fn run_sync(
    db_path: std::path::PathBuf,
    syncing: Arc<AtomicBool>,
    urls: Vec<String>,
    user_email: Option<String>,
) {
    let mut conn = match rusqlite::Connection::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            syncing.store(false, Ordering::Release);
            return;
        }
    };
    // WAL mode must be set on each new connection.
    let _ = conn.execute_batch("PRAGMA journal_mode=WAL;");

    let today = Local::now().date_naive();
    let window_start = today - Duration::days(365 * 3);
    let window_end = today + Duration::days(365);

    let mut last_error: Option<String> = None;

    let email_lower = user_email
        .as_deref()
        .map(|e| e.trim().to_lowercase())
        .filter(|e| !e.is_empty());

    for url in &urls {
        if let Err(e) = sync_one_url(&mut conn, url.trim(), window_start, window_end, &email_lower) {
            last_error = Some(e);
        }
    }

    let ts = now();
    let _ = conn.execute(
        "INSERT OR REPLACE INTO ical_sync_meta (id, last_synced_at, last_error) VALUES (1, ?1, ?2)",
        params![ts, last_error],
    );
    // Invalidate the day cache so freshly synced events appear.
    let _ = conn.execute("DELETE FROM timeline_day_cache", []);

    syncing.store(false, Ordering::Release);
}

fn sync_one_url(
    conn: &mut rusqlite::Connection,
    url: &str,
    window_start: NaiveDate,
    window_end: NaiveDate,
    user_email: &Option<String>,
) -> Result<(), String> {
    let mut resp = ureq::get(url)
        .header("Accept", "text/calendar")
        .call()
        .map_err(|e| format!("iCal fetch: {e}"))?;
    let content = resp.body_mut().read_to_string().map_err(|e| format!("iCal read: {e}"))?;

    let events = parse_and_expand(&content, window_start, window_end, user_email);

    // A single transaction makes thousands of inserts orders of magnitude faster.
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM ical_events WHERE url = ?1", params![url])
        .map_err(|e| e.to_string())?;
    for ev in &events {
        tx.execute(
            "INSERT OR REPLACE INTO ical_events (url, uid, dtstart, summary, event_url, dtend, declined)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![url, ev.uid, ev.dtstart, ev.summary, ev.event_url, ev.dtend, ev.declined as i64],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// ── Parsing ────────────────────────────────────────────────────────────────────

struct ParsedEvent {
    uid: String,
    summary: String,
    event_url: Option<String>,
    dtstart_val: String,
    dtstart_tzid: Option<String>,
    dtend_val: Option<String>,
    is_all_day: bool,
    rrule: Option<String>,
    exdates: Vec<String>,
    recurrence_id_ts: Option<i64>,
    declined: bool,
}

struct ExpandedEvent {
    uid: String,
    dtstart: i64,
    dtend: Option<i64>,
    summary: String,
    event_url: Option<String>,
    declined: bool,
}

fn parse_events_from_ics(body: &str, user_email: &Option<String>) -> Vec<ParsedEvent> {
    let reader = BufReader::new(body.as_bytes());
    let parser = IcalParser::new(reader);
    let mut events = Vec::new();

    for calendar in parser.flatten() {
        for event in &calendar.events {
            let get = |name: &str| -> Option<String> {
                event
                    .properties
                    .iter()
                    .find(|p| p.name == name)
                    .and_then(|p| p.value.clone())
            };

            let dtstart_prop = event.properties.iter().find(|p| p.name == "DTSTART");
            let dtstart_val = match dtstart_prop.and_then(|p| p.value.as_deref()) {
                Some(v) => v.to_string(),
                None => continue,
            };

            let dtstart_tzid = dtstart_prop
                .and_then(|p| p.params.as_ref())
                .and_then(|ps| ps.iter().find(|(k, _)| k == "TZID"))
                .and_then(|(_, v)| v.first().cloned());

            let is_all_day = dtstart_prop
                .and_then(|p| p.params.as_ref())
                .and_then(|ps| ps.iter().find(|(k, _)| k == "VALUE"))
                .map(|(_, v)| v.iter().any(|s| s == "DATE"))
                .unwrap_or(false)
                || !dtstart_val.contains('T');

            let dtend_val = get("DTEND");

            let exdates: Vec<String> = event
                .properties
                .iter()
                .filter(|p| p.name == "EXDATE")
                .filter_map(|p| p.value.clone())
                .collect();

            let recurrence_id_ts = get("RECURRENCE-ID")
                .as_deref()
                .and_then(parse_ics_datetime)
                .map(|dt| dt.timestamp());

            let uid = get("UID").unwrap_or_else(|| format!("no-uid:{dtstart_val}"));

            // Declined: true if the user's ATTENDEE entry has PARTSTAT=DECLINED.
            let declined = match user_email {
                Some(email) => event
                    .properties
                    .iter()
                    .filter(|p| p.name == "ATTENDEE")
                    .any(|p| {
                        let value_matches = p
                            .value
                            .as_deref()
                            .map(|v| v.to_lowercase().contains(email.as_str()))
                            .unwrap_or(false);
                        let partstat_declined = p
                            .params
                            .as_ref()
                            .and_then(|ps| ps.iter().find(|(k, _)| k == "PARTSTAT"))
                            .map(|(_, v)| v.iter().any(|s| s == "DECLINED"))
                            .unwrap_or(false);
                        value_matches && partstat_declined
                    }),
                None => false,
            };

            events.push(ParsedEvent {
                uid,
                summary: get("SUMMARY").unwrap_or_else(|| "(No title)".into()),
                event_url: get("URL"),
                dtstart_val,
                dtstart_tzid,
                dtend_val,
                is_all_day,
                rrule: get("RRULE"),
                exdates,
                recurrence_id_ts,
                declined,
            });
        }
    }

    events
}

fn parse_and_expand(
    body: &str,
    window_start: NaiveDate,
    window_end: NaiveDate,
    user_email: &Option<String>,
) -> Vec<ExpandedEvent> {
    let parsed = parse_events_from_ics(body, user_email);

    // Collect RECURRENCE-ID overrides per uid so we can skip those master occurrences.
    let mut overridden_ts: HashMap<String, HashSet<i64>> = HashMap::new();
    for ev in &parsed {
        if let Some(ts) = ev.recurrence_id_ts {
            overridden_ts.entry(ev.uid.clone()).or_default().insert(ts);
        }
    }

    let mut results: Vec<ExpandedEvent> = Vec::new();

    for ev in &parsed {
        if ev.is_all_day {
            continue;
        }

        let dtend_ts = ev.dtend_val.as_deref().and_then(parse_ics_datetime).map(|d| d.timestamp());

        if let Some(rrule_val) = &ev.rrule {
            let overrides = overridden_ts.get(&ev.uid);
            let occurrences = expand_rrule_for_range(
                &ev.dtstart_val,
                ev.dtstart_tzid.as_deref(),
                rrule_val,
                &ev.exdates,
                window_start,
                window_end,
            );
            for dt in occurrences {
                let ts = dt.timestamp();
                if overrides.map(|s| s.contains(&ts)).unwrap_or(false) {
                    continue;
                }
                // For recurring events, shift dtend by the same offset as dtstart.
                let occurrence_dtend = dtend_ts.and_then(|end| {
                    let dtstart_ts = parse_ics_datetime(&ev.dtstart_val)?.timestamp();
                    Some(end - dtstart_ts + ts)
                });
                results.push(ExpandedEvent {
                    uid: format!("{}:{}", ev.uid, ts),
                    dtstart: ts,
                    dtend: occurrence_dtend,
                    summary: ev.summary.clone(),
                    event_url: ev.event_url.clone(),
                    declined: ev.declined,
                });
            }
        } else {
            let dt = match parse_ics_datetime(&ev.dtstart_val) {
                Some(d) => d,
                None => continue,
            };
            let date = dt.date_naive();
            if date < window_start || date > window_end {
                continue;
            }
            results.push(ExpandedEvent {
                uid: ev.uid.clone(),
                dtstart: dt.timestamp(),
                dtend: dtend_ts,
                summary: ev.summary.clone(),
                event_url: ev.event_url.clone(),
                declined: ev.declined,
            });
        }
    }

    results
}

fn expand_rrule_for_range(
    dtstart_val: &str,
    dtstart_tzid: Option<&str>,
    rrule_val: &str,
    exdates: &[String],
    window_start: NaiveDate,
    window_end: NaiveDate,
) -> Vec<DateTime<Local>> {
    use rrule::RRuleSet;

    let tz_prefix = dtstart_tzid
        .map(|tz| format!(";TZID={tz}"))
        .unwrap_or_default();

    let mut ical_str = format!("DTSTART{tz_prefix}:{dtstart_val}\nRRULE:{rrule_val}\n");
    for exdate in exdates {
        ical_str.push_str(&format!("EXDATE:{exdate}\n"));
    }

    let rrule_set: RRuleSet = match ical_str.parse() {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    let start_local = match Local
        .from_local_datetime(&window_start.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
    {
        Some(dt) => dt,
        None => return vec![],
    };
    let end_local = match Local
        .from_local_datetime(&window_end.and_hms_opt(23, 59, 59).unwrap())
        .earliest()
    {
        Some(dt) => dt,
        None => return vec![],
    };

    let after_tz: DateTime<rrule::Tz> = start_local.with_timezone(&rrule::Tz::UTC);
    let before_tz: DateTime<rrule::Tz> = end_local.with_timezone(&rrule::Tz::UTC);

    let result = rrule_set.after(after_tz).before(before_tz).all(5000);
    if result.limited {
        // Hit the 5000-occurrence cap — the window very likely has more
        // matching dates that we silently dropped. Surface it so a user
        // with e.g. an hourly recurrence isn't left wondering why late
        // events are missing.
        eprintln!(
            "[iCal] RRULE expansion hit the 5000-occurrence cap for DTSTART={dtstart_val}; \
             later occurrences in the window were truncated."
        );
    }
    result
        .dates
        .into_iter()
        .map(|dt| Utc.from_utc_datetime(&dt.naive_utc()).with_timezone(&Local))
        .collect()
}

pub(crate) fn parse_ics_datetime(value: &str) -> Option<DateTime<Local>> {
    if let Some(without_z) = value.strip_suffix('Z') {
        let naive = NaiveDateTime::parse_from_str(without_z, "%Y%m%dT%H%M%S").ok()?;
        Some(Utc.from_utc_datetime(&naive).with_timezone(&Local))
    } else {
        let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
        Local.from_local_datetime(&naive).earliest()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> (NaiveDate, NaiveDate) {
        (
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
        )
    }

    // --- parse_ics_datetime ---

    #[test]
    fn parse_datetime_utc_z_suffix() {
        let dt = parse_ics_datetime("20240115T100000Z").expect("Should parse UTC datetime");
        let expected = Utc
            .with_ymd_and_hms(2024, 1, 15, 10, 0, 0)
            .unwrap()
            .timestamp();
        assert_eq!(dt.timestamp(), expected);
    }

    #[test]
    fn parse_datetime_local_no_suffix() {
        assert!(parse_ics_datetime("20240115T143000").is_some());
    }

    #[test]
    fn parse_datetime_invalid_returns_none() {
        assert!(parse_ics_datetime("not-a-datetime").is_none());
        assert!(parse_ics_datetime("").is_none());
    }

    // --- parse_and_expand with inline ICS feeds ---

    #[test]
    fn simple_event_appears_with_correct_duration() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\n\
                   BEGIN:VEVENT\r\nUID:simple@test\r\nSUMMARY:Team Standup\r\n\
                   DTSTART:20240115T100000Z\r\nDTEND:20240115T103000Z\r\n\
                   END:VEVENT\r\nEND:VCALENDAR\r\n";

        let (ws, we) = window();
        let events = parse_and_expand(ics, ws, we, &None);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].summary, "Team Standup");
        assert_eq!(events[0].uid, "simple@test");
        assert!(!events[0].declined);
        // DTEND - DTSTART = 30 minutes = 1800 seconds
        assert_eq!(events[0].dtend.unwrap() - events[0].dtstart, 1800);
    }

    #[test]
    fn all_day_event_is_excluded() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\n\
                   BEGIN:VEVENT\r\nUID:allday@test\r\nSUMMARY:Holiday\r\n\
                   DTSTART;VALUE=DATE:20240115\r\nDTEND;VALUE=DATE:20240116\r\n\
                   END:VEVENT\r\nEND:VCALENDAR\r\n";

        let (ws, we) = window();
        let events = parse_and_expand(ics, ws, we, &None);
        assert!(events.is_empty(), "All-day events must be excluded");
    }

    #[test]
    fn declined_attendee_event_is_marked_declined() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\n\
                   BEGIN:VEVENT\r\nUID:declined@test\r\nSUMMARY:Party\r\n\
                   DTSTART:20240115T180000Z\r\nDTEND:20240115T200000Z\r\n\
                   ATTENDEE;PARTSTAT=DECLINED:mailto:user@example.com\r\n\
                   END:VEVENT\r\nEND:VCALENDAR\r\n";

        let (ws, we) = window();
        let email = Some("user@example.com".to_string());
        let events = parse_and_expand(ics, ws, we, &email);

        assert_eq!(events.len(), 1);
        assert!(events[0].declined, "Event should be marked declined");
    }

    #[test]
    fn accepted_attendee_event_is_not_declined() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\n\
                   BEGIN:VEVENT\r\nUID:accepted@test\r\nSUMMARY:Meeting\r\n\
                   DTSTART:20240115T100000Z\r\nDTEND:20240115T110000Z\r\n\
                   ATTENDEE;PARTSTAT=ACCEPTED:mailto:user@example.com\r\n\
                   END:VEVENT\r\nEND:VCALENDAR\r\n";

        let (ws, we) = window();
        let email = Some("user@example.com".to_string());
        let events = parse_and_expand(ics, ws, we, &email);

        assert_eq!(events.len(), 1);
        assert!(!events[0].declined);
    }

    #[test]
    fn recurring_weekly_event_expands_all_occurrences() {
        // RRULE:FREQ=WEEKLY;COUNT=4 starting Jan 1 → Jan 1, 8, 15, 22, all inside window
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\n\
                   BEGIN:VEVENT\r\nUID:weekly@test\r\nSUMMARY:Weekly Sync\r\n\
                   DTSTART:20240101T140000Z\r\nDTEND:20240101T150000Z\r\n\
                   RRULE:FREQ=WEEKLY;COUNT=4\r\n\
                   END:VEVENT\r\nEND:VCALENDAR\r\n";

        let (ws, we) = window();
        let events = parse_and_expand(ics, ws, we, &None);

        assert_eq!(events.len(), 4, "Should expand to 4 weekly occurrences");
        assert!(events.iter().all(|e| e.summary == "Weekly Sync"));
    }

    #[test]
    fn exdate_excludes_one_occurrence() {
        // 3 weekly occurrences (Jan 1, 8, 15) minus EXDATE Jan 8 → 2 events
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\n\
                   BEGIN:VEVENT\r\nUID:exc@test\r\nSUMMARY:Weekly with Gap\r\n\
                   DTSTART:20240101T100000Z\r\nDTEND:20240101T110000Z\r\n\
                   RRULE:FREQ=WEEKLY;COUNT=3\r\nEXDATE:20240108T100000Z\r\n\
                   END:VEVENT\r\nEND:VCALENDAR\r\n";

        let (ws, we) = window();
        let events = parse_and_expand(ics, ws, we, &None);

        assert_eq!(events.len(), 2, "EXDATE should remove one occurrence");
    }

    #[test]
    fn event_outside_window_is_excluded() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\n\
                   BEGIN:VEVENT\r\nUID:outside@test\r\nSUMMARY:March Event\r\n\
                   DTSTART:20240301T100000Z\r\nDTEND:20240301T110000Z\r\n\
                   END:VEVENT\r\nEND:VCALENDAR\r\n";

        let (ws, we) = window();
        let events = parse_and_expand(ics, ws, we, &None);
        assert!(events.is_empty(), "Event outside window should be excluded");
    }

    #[test]
    fn event_url_is_preserved() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\n\
                   BEGIN:VEVENT\r\nUID:url@test\r\nSUMMARY:Meeting\r\n\
                   DTSTART:20240115T100000Z\r\nDTEND:20240115T110000Z\r\n\
                   URL:https://meet.example.com/room123\r\n\
                   END:VEVENT\r\nEND:VCALENDAR\r\n";

        let (ws, we) = window();
        let events = parse_and_expand(ics, ws, we, &None);

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].event_url.as_deref(),
            Some("https://meet.example.com/room123")
        );
    }
}
