use chrono::{Local, NaiveDate, TimeZone};
use rusqlite::params;
use tauri::State;

use crate::commands::settings_ical::load_settings_ical;
use crate::state::AppState;
use crate::timeline::{sanitize_event_url, TimelineEvent, TimelineEventSource};

pub(super) fn test_connection(state: &State<'_, AppState>) -> Result<(), String> {
    let settings = load_settings_ical(state);
    let urls: Vec<String> = settings
        .urls
        .into_iter()
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
        .collect();
    if urls.is_empty() {
        return Err("At least one iCal URL is required".into());
    }
    for url in &urls {
        match ureq::get(url).header("Accept", "text/calendar").call() {
            Ok(mut r) => {
                let body = r
                    .body_mut()
                    .read_to_string()
                    .map_err(|e| format!("iCal read failed for {url}: {e}"))?;
                if !body.contains("BEGIN:VCALENDAR") {
                    return Err(format!(
                        "URL did not return an iCal feed (missing BEGIN:VCALENDAR): {url}"
                    ));
                }
            }
            Err(ureq::Error::StatusCode(status)) => {
                return Err(format!("iCal URL returned HTTP {status}: {url}"));
            }
            Err(e) => return Err(format!("iCal request failed for {url}: {e}")),
        }
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
    Ok(rows
        .into_iter()
        .filter(|(d, _, _)| *d == day_naive)
        .map(|(_, ts, ev)| (ts, ev))
        .collect())
}

pub(super) fn events_for_range(
    state: &State<'_, AppState>,
    start_day: NaiveDate,
    end_day: NaiveDate,
) -> Result<Vec<(NaiveDate, i64, TimelineEvent)>, String> {
    let settings = load_settings_ical(state);
    if !settings.enabled || settings.urls.is_empty() {
        return Ok(Vec::new());
    }

    let next_end = end_day
        .succ_opt()
        .ok_or_else(|| format!("no day after {end_day}"))?;
    let range_start = Local
        .from_local_datetime(&start_day.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .map(|dt| dt.timestamp())
        .ok_or("Invalid range start")?;
    // Exclusive upper bound: midnight after end_day.
    let range_end = Local
        .from_local_datetime(&next_end.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .map(|dt| dt.timestamp())
        .ok_or("Invalid range end")?;

    let conn = state.db.lock().map_err(|_| "DB lock failed".to_string())?;
    // Match events that start inside the range OR start earlier and run into
    // it (overnight events like "On-call Mon 18:00 → Tue 09:00" need to show
    // on every day they cover).
    let mut stmt = conn
        .prepare(
            "SELECT uid, dtstart, dtend, summary, event_url
             FROM ical_events
             WHERE (declined IS NULL OR declined = 0)
               AND (
                 (dtstart >= ?1 AND dtstart < ?2)
                 OR (dtend IS NOT NULL AND dtstart < ?1 AND dtend > ?1)
               )",
        )
        .map_err(|e| e.to_string())?;

    type IcalRow = (String, i64, Option<i64>, String, Option<String>);
    let raw: Vec<IcalRow> = stmt
        .query_map(params![range_start, range_end], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    drop(conn);

    let mut results: Vec<(NaiveDate, i64, TimelineEvent)> = Vec::new();
    for (uid, dtstart, dtend, summary, event_url) in raw {
        let detail = dtend.and_then(|end| {
            let mins = (end - dtstart) / 60;
            if mins <= 0 {
                return None;
            }
            let h = mins / 60;
            let m = mins % 60;
            Some(match (h, m) {
                (0, m) => format!("{m}m"),
                (h, 0) => format!("{h}h"),
                (h, m) => format!("{h}h {m}m"),
            })
        });

        use chrono::DateTime;
        let start_dt = DateTime::from_timestamp(dtstart, 0)
            .map(|d| d.with_timezone(&Local))
            .unwrap_or_else(chrono::Local::now);
        let start_day_local = start_dt.date_naive();

        // Enumerate every local day the event covers, clamped to [start_day, end_day].
        let effective_end_ts = dtend.unwrap_or(dtstart);
        let effective_end_dt = DateTime::from_timestamp(effective_end_ts.max(dtstart), 0)
            .map(|d| d.with_timezone(&Local))
            .unwrap_or(start_dt);
        let mut cover_end_day = effective_end_dt.date_naive();
        // An event ending exactly at midnight doesn't cover the next day.
        if dtend.is_some() && effective_end_dt.time() == chrono::NaiveTime::MIN && cover_end_day > start_day_local {
            cover_end_day = cover_end_day.pred_opt().unwrap_or(cover_end_day);
        }

        let first = start_day_local.max(start_day);
        let last = cover_end_day.min(end_day);
        if first > last {
            continue;
        }

        let mut d = first;
        loop {
            // For overnight events carrying into a day, clamp displayed
            // time to "00:00"; the original dtstart remains the sort key
            // so these events appear at the top of that day.
            let time = if d == start_day_local {
                start_dt.format("%H:%M").to_string()
            } else {
                "00:00".to_string()
            };
            results.push((
                d,
                dtstart,
                TimelineEvent {
                    id: format!("calendar:{uid}"),
                    time,
                    timestamp: dtstart,
                    source: TimelineEventSource::Calendar,
                    title: summary.clone(),
                    detail: detail.clone(),
                    url: event_url.as_deref().and_then(sanitize_event_url),
                },
            ));
            if d == last {
                break;
            }
            let Some(next) = d.succ_opt() else { break };
            d = next;
        }
    }

    results.sort_by_key(|(_, ts, _)| *ts);
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::settings_ical::KEY as ICAL_KEY;
    use crate::test_support::{local_ts, mock_app, seed_setting, state};

    /// A Tuesday with no DST transition anywhere nearby.
    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 3, 5).unwrap()
    }

    fn enable(state: &State<'_, AppState>) {
        seed_setting(
            state,
            ICAL_KEY,
            r#"{"enabled":true,"urls":["https://cal.example/feed.ics"],"emails":[]}"#,
        );
    }

    fn insert(
        state: &State<'_, AppState>,
        uid: &str,
        dtstart: i64,
        dtend: Option<i64>,
        summary: &str,
        url: Option<&str>,
        declined: bool,
    ) {
        let conn = state.db.lock().unwrap();
        conn.execute(
            "INSERT INTO ical_events (url, uid, dtstart, dtend, summary, event_url, declined)
             VALUES ('https://cal.example/feed.ics', ?1, ?2, ?3, ?4, ?5, ?6)",
            params![uid, dtstart, dtend, summary, url, declined as i64],
        )
        .unwrap();
    }

    #[test]
    fn disabled_source_returns_nothing_even_with_rows() {
        let app = mock_app();
        let s = state(&app);
        insert(&s, "u1", local_ts(day(), 10, 0), None, "Standup", None, false);
        assert!(events_for_range(&s, day(), day()).unwrap().is_empty());

        seed_setting(&s, ICAL_KEY, r#"{"enabled":false,"urls":["https://x"],"emails":[]}"#);
        assert!(events_for_range(&s, day(), day()).unwrap().is_empty());
    }

    #[test]
    fn enabled_without_urls_returns_nothing() {
        let app = mock_app();
        let s = state(&app);
        seed_setting(&s, ICAL_KEY, r#"{"enabled":true,"urls":[],"emails":[]}"#);
        insert(&s, "u1", local_ts(day(), 10, 0), None, "Standup", None, false);
        assert!(events_for_range(&s, day(), day()).unwrap().is_empty());
    }

    #[test]
    fn an_event_maps_to_one_timeline_row() {
        let app = mock_app();
        let s = state(&app);
        enable(&s);
        let start = local_ts(day(), 10, 0);
        insert(
            &s,
            "u1",
            start,
            Some(start + 30 * 60),
            "Standup",
            Some("https://calendar.example/event/1"),
            false,
        );

        let rows = events_for_range(&s, day(), day()).unwrap();
        assert_eq!(rows.len(), 1);
        let (d, ts, ev) = &rows[0];
        assert_eq!(*d, day());
        assert_eq!(*ts, start);
        assert_eq!(ev.id, "calendar:u1");
        assert_eq!(ev.time, "10:00");
        assert_eq!(ev.timestamp, start);
        assert_eq!(ev.source, TimelineEventSource::Calendar);
        assert_eq!(ev.title, "Standup");
        assert_eq!(ev.detail.as_deref(), Some("30m"));
        assert_eq!(ev.url.as_deref(), Some("https://calendar.example/event/1"));
    }

    #[test]
    fn duration_detail_formats() {
        let app = mock_app();
        let s = state(&app);
        enable(&s);
        let base = local_ts(day(), 8, 0);
        insert(&s, "h1", base, Some(base + 3600), "1h", None, false);
        insert(&s, "h1m30", base + 1, Some(base + 1 + 90 * 60), "1h 30m", None, false);
        insert(&s, "m45", base + 2, Some(base + 2 + 45 * 60), "45m", None, false);
        insert(&s, "noend", base + 3, None, "no end", None, false);
        insert(&s, "zero", base + 4, Some(base + 4), "zero length", None, false);
        insert(&s, "neg", base + 5, Some(base), "ends before start", None, false);

        let rows = events_for_range(&s, day(), day()).unwrap();
        let by_title: std::collections::HashMap<_, _> =
            rows.iter().map(|(_, _, ev)| (ev.title.as_str(), ev.detail.clone())).collect();
        assert_eq!(by_title["1h"].as_deref(), Some("1h"));
        assert_eq!(by_title["1h 30m"].as_deref(), Some("1h 30m"));
        assert_eq!(by_title["45m"].as_deref(), Some("45m"));
        assert_eq!(by_title["no end"], None);
        assert_eq!(by_title["zero length"], None);
        assert_eq!(by_title["ends before start"], None);
    }

    #[test]
    fn declined_events_are_excluded() {
        let app = mock_app();
        let s = state(&app);
        enable(&s);
        insert(&s, "yes", local_ts(day(), 9, 0), None, "Going", None, false);
        insert(&s, "no", local_ts(day(), 10, 0), None, "Declined", None, true);
        let rows = events_for_range(&s, day(), day()).unwrap();
        assert_eq!(rows.iter().map(|(_, _, e)| e.title.as_str()).collect::<Vec<_>>(), vec!["Going"]);
    }

    #[test]
    fn non_http_event_urls_are_dropped() {
        let app = mock_app();
        let s = state(&app);
        enable(&s);
        insert(&s, "js", local_ts(day(), 9, 0), None, "js", Some("javascript:alert(1)"), false);
        insert(&s, "ok", local_ts(day(), 10, 0), None, "ok", Some("HTTPS://ok.example/e"), false);
        let rows = events_for_range(&s, day(), day()).unwrap();
        assert_eq!(rows[0].2.url, None);
        assert_eq!(rows[1].2.url.as_deref(), Some("HTTPS://ok.example/e"));
    }

    #[test]
    fn an_overnight_event_appears_on_both_days() {
        let app = mock_app();
        let s = state(&app);
        enable(&s);
        let next = day().succ_opt().unwrap();
        let start = local_ts(day(), 18, 0);
        insert(&s, "oncall", start, Some(local_ts(next, 9, 0)), "On-call", None, false);

        let rows = events_for_range(&s, day(), next).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].0, rows[0].2.time.as_str()), (day(), "18:00"));
        assert_eq!((rows[1].0, rows[1].2.time.as_str()), (next, "00:00"));
        // Both rows sort by the real start so the carried-over row leads its day.
        assert_eq!(rows[0].1, start);
        assert_eq!(rows[1].1, start);
        assert_eq!(rows[1].2.timestamp, start);
        assert_eq!(rows[1].2.detail.as_deref(), Some("15h"));
    }

    #[test]
    fn an_overnight_event_is_found_when_the_range_starts_on_its_second_day() {
        let app = mock_app();
        let s = state(&app);
        enable(&s);
        let next = day().succ_opt().unwrap();
        insert(&s, "oncall", local_ts(day(), 18, 0), Some(local_ts(next, 9, 0)), "On-call", None, false);

        let rows = events_for_range(&s, next, next).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, next);
        assert_eq!(rows[0].2.time, "00:00");
    }

    #[test]
    fn an_event_ending_exactly_at_midnight_does_not_spill_into_the_next_day() {
        let app = mock_app();
        let s = state(&app);
        enable(&s);
        let next = day().succ_opt().unwrap();
        insert(&s, "late", local_ts(day(), 22, 0), Some(local_ts(next, 0, 0)), "Late", None, false);

        let rows = events_for_range(&s, day(), next).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, day());
        assert!(events_for_range(&s, next, next).unwrap().is_empty());
    }

    #[test]
    fn events_outside_the_range_are_excluded() {
        let app = mock_app();
        let s = state(&app);
        enable(&s);
        let before = day().pred_opt().unwrap();
        let after = day().succ_opt().unwrap().succ_opt().unwrap();
        insert(&s, "b", local_ts(before, 23, 59), Some(local_ts(before, 23, 59) + 30), "before", None, false);
        insert(&s, "a", local_ts(after, 0, 0), None, "after", None, false);
        insert(&s, "in", local_ts(day(), 0, 0), None, "midnight start counts", None, false);

        let rows = events_for_range(&s, day(), day()).unwrap();
        assert_eq!(rows.iter().map(|(_, _, e)| e.title.as_str()).collect::<Vec<_>>(), vec!["midnight start counts"]);
    }

    #[test]
    fn results_are_sorted_by_start_time_regardless_of_insert_order() {
        let app = mock_app();
        let s = state(&app);
        enable(&s);
        insert(&s, "pm", local_ts(day(), 14, 0), None, "pm", None, false);
        insert(&s, "am", local_ts(day(), 9, 0), None, "am", None, false);
        insert(&s, "noon", local_ts(day(), 12, 0), None, "noon", None, false);
        let rows = events_for_range(&s, day(), day()).unwrap();
        assert_eq!(rows.iter().map(|(_, _, e)| e.time.as_str()).collect::<Vec<_>>(), vec!["09:00", "12:00", "14:00"]);
    }

    #[test]
    fn events_for_day_returns_only_that_day() {
        let app = mock_app();
        let s = state(&app);
        enable(&s);
        let next = day().succ_opt().unwrap();
        insert(&s, "oncall", local_ts(day(), 18, 0), Some(local_ts(next, 9, 0)), "On-call", None, false);
        insert(&s, "later", local_ts(next, 11, 0), None, "Later", None, false);

        let first = events_for_day(&s, "2024-03-05").unwrap();
        assert_eq!(first.iter().map(|(_, e)| e.time.as_str()).collect::<Vec<_>>(), vec!["18:00"]);
        let second = events_for_day(&s, "2024-03-06").unwrap();
        assert_eq!(second.iter().map(|(_, e)| e.title.as_str()).collect::<Vec<_>>(), vec!["On-call", "Later"]);
        assert_eq!(second[0].1.time, "00:00");
    }

    #[test]
    fn events_for_day_rejects_a_malformed_date() {
        let app = mock_app();
        let s = state(&app);
        enable(&s);
        let err = events_for_day(&s, "05/03/2024").unwrap_err();
        assert!(err.contains("Invalid date"), "{err}");
    }
}
