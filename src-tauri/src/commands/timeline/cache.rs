//! SQLite-backed cache for a full day's merged timeline events.
//!
//! We only cache days that have already ended (strictly before the local "today"),
//! so the current day and future days always go through the live data-source fetch.
//! This keeps "reopen the app and see your last week" fast, while today's view
//! stays fresh as activity lands.

use rusqlite::params;
use tauri::State;

use crate::commands::settings::now;
use crate::state::AppState;
use crate::timeline::TimelineEvent;

/// Maximum number of cached days to retain. One row ≈ a day of events
/// (typically small JSON), so this is a soft upper bound that prevents the
/// table from growing forever over years of use. Two calendar years of
/// coverage comfortably spans any realistic Harvest-entry backfill.
const MAX_CACHED_DAYS: i64 = 730;

pub(super) fn get_cached_day(
    state: &State<'_, AppState>,
    day: &str,
) -> Option<Vec<TimelineEvent>> {
    let conn = state.db.lock().ok()?;
    let mut stmt = conn
        .prepare("SELECT events_json FROM timeline_day_cache WHERE day = ?1")
        .ok()?;
    let json: String = stmt.query_row(params![day], |row| row.get(0)).ok()?;
    serde_json::from_str(&json).ok()
}

pub(super) fn save_cached_day(
    state: &State<'_, AppState>,
    day: &str,
    events: &[TimelineEvent],
) -> Result<(), String> {
    let json = serde_json::to_string(events).map_err(|e| format!("serialize cache: {e}"))?;
    let conn = state.db.lock().map_err(|_| "Failed to access database")?;
    conn.execute(
        "INSERT OR REPLACE INTO timeline_day_cache (day, events_json, updated_at) VALUES (?1, ?2, ?3)",
        params![day, json, now()],
    )
    .map_err(|e| e.to_string())?;

    // Evict the oldest-by-day rows once we exceed the cap so the table
    // doesn't grow without bound. `day` is a YYYY-MM-DD string, so a
    // lexicographic sort matches chronological order.
    conn.execute(
        "DELETE FROM timeline_day_cache WHERE day NOT IN (
             SELECT day FROM timeline_day_cache ORDER BY day DESC LIMIT ?1
         )",
        params![MAX_CACHED_DAYS],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{event, mock_app, state};
    use crate::timeline::TimelineEventSource;
    use chrono::NaiveDate;

    fn row_count(state: &State<'_, AppState>) -> i64 {
        let conn = state.db.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM timeline_day_cache", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn missing_day_is_none() {
        let app = mock_app();
        assert!(get_cached_day(&state(&app), "2024-03-05").is_none());
    }

    #[test]
    fn roundtrip_preserves_events_and_their_order() {
        let app = mock_app();
        let s = state(&app);
        let mut a = event("git:/r:aaa", 1_709_632_800, TimelineEventSource::Git);
        a.detail = Some("recall — aaa".into());
        a.url = Some("https://example.com/aaa".into());
        let b = event("jira:DDF-1:2024-03-05:Commented", 1_709_629_200, TimelineEventSource::Jira);
        // Deliberately not sorted: the cache stores what it is given.
        save_cached_day(&s, "2024-03-05", &[a.clone(), b.clone()]).unwrap();

        let cached = get_cached_day(&s, "2024-03-05").expect("cached");
        assert_eq!(cached.len(), 2);
        assert_eq!(cached[0].id, a.id);
        assert_eq!(cached[0].detail, a.detail);
        assert_eq!(cached[0].url, a.url);
        assert_eq!(cached[0].timestamp, a.timestamp);
        assert_eq!(cached[0].source, TimelineEventSource::Git);
        assert_eq!(cached[1].id, b.id);
        assert_eq!(cached[1].source, TimelineEventSource::Jira);
    }

    #[test]
    fn an_empty_day_is_cached_as_some_empty_vec() {
        // "No activity" must be distinguishable from "never fetched", or the
        // month heatmap would re-fetch quiet days forever.
        let app = mock_app();
        let s = state(&app);
        save_cached_day(&s, "2024-03-05", &[]).unwrap();
        let cached = get_cached_day(&s, "2024-03-05").expect("an empty day is still a cache hit");
        assert!(cached.is_empty());
    }

    #[test]
    fn saving_the_same_day_again_replaces_it() {
        let app = mock_app();
        let s = state(&app);
        save_cached_day(&s, "2024-03-05", &[event("a", 1, TimelineEventSource::Git)]).unwrap();
        save_cached_day(&s, "2024-03-05", &[event("b", 2, TimelineEventSource::Zulip)]).unwrap();
        let cached = get_cached_day(&s, "2024-03-05").unwrap();
        assert_eq!(cached.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["b"]);
        assert_eq!(row_count(&s), 1);
    }

    #[test]
    fn days_are_independent() {
        let app = mock_app();
        let s = state(&app);
        save_cached_day(&s, "2024-03-05", &[event("a", 1, TimelineEventSource::Git)]).unwrap();
        save_cached_day(&s, "2024-03-06", &[]).unwrap();
        assert_eq!(get_cached_day(&s, "2024-03-05").unwrap().len(), 1);
        assert_eq!(get_cached_day(&s, "2024-03-06").unwrap().len(), 0);
        assert!(get_cached_day(&s, "2024-03-07").is_none());
    }

    #[test]
    fn a_corrupt_row_reads_as_missing() {
        let app = mock_app();
        let s = state(&app);
        {
            let conn = s.db.lock().unwrap();
            conn.execute(
                "INSERT INTO timeline_day_cache (day, events_json, updated_at) VALUES ('2024-03-05', '{not json', 1)",
                [],
            )
            .unwrap();
        }
        assert!(get_cached_day(&s, "2024-03-05").is_none());
    }

    #[test]
    fn evicts_the_oldest_days_beyond_the_cap() {
        let app = mock_app();
        let s = state(&app);
        let first = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        let mut day = first;
        for _ in 0..(MAX_CACHED_DAYS + 1) {
            save_cached_day(&s, &day.format("%Y-%m-%d").to_string(), &[]).unwrap();
            day = day.succ_opt().unwrap();
        }
        let last = day.pred_opt().unwrap();

        assert_eq!(row_count(&s), MAX_CACHED_DAYS);
        assert!(get_cached_day(&s, "2020-01-01").is_none(), "oldest day evicted");
        assert!(get_cached_day(&s, "2020-01-02").is_some(), "second-oldest survives");
        assert!(get_cached_day(&s, &last.format("%Y-%m-%d").to_string()).is_some());
    }

    #[test]
    fn eviction_is_by_calendar_day_not_insertion_time() {
        // Once full, writing a day older than everything cached evicts that
        // very row: the cap keeps the newest calendar days, not the most
        // recently written ones.
        let app = mock_app();
        let s = state(&app);
        let mut day = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        for _ in 0..MAX_CACHED_DAYS {
            save_cached_day(&s, &day.format("%Y-%m-%d").to_string(), &[]).unwrap();
            day = day.succ_opt().unwrap();
        }
        assert_eq!(row_count(&s), MAX_CACHED_DAYS);

        save_cached_day(&s, "2019-12-31", &[]).unwrap();
        assert_eq!(row_count(&s), MAX_CACHED_DAYS);
        assert!(get_cached_day(&s, "2019-12-31").is_none());
        assert!(get_cached_day(&s, "2020-01-01").is_some());
    }
}
