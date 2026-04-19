use crate::commands::settings::{get_val, save_val};
use crate::state::AppState;
use ical::IcalParser;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::io::BufReader;
use tauri::State;

pub(crate) const KEY: &str = "settings_ical";

#[derive(Deserialize, Serialize, Type, Default)]
#[specta(export = false)]
pub struct SettingsIcal {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub urls: Vec<String>,
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
    let json = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    save_val(&state, KEY, &json)?;
    // Invalidate the day cache so past days are re-fetched with calendar events.
    let conn = state.db.lock().map_err(|_| "DB lock failed".to_string())?;
    conn.execute("DELETE FROM timeline_day_cache", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize, Type)]
pub struct IcalDebugInfo {
    pub url: String,
    pub error: Option<String>,
    pub total_events: u32,
    pub all_day_skipped: u32,
    pub recurring_rrule: u32,
    /// Events on today's date, formatted as "HH:MM — Title".
    pub today_events: Vec<String>,
    /// First few raw DTSTART values from the feed, for diagnosing parse failures.
    pub dtstart_samples: Vec<String>,
}

/// Fetches `url` fresh (skipping cache) and stores the result so the timeline benefits.
fn fetch_ical_fresh(state: &State<'_, AppState>, url: &str) -> Result<String, String> {
    use crate::commands::settings::now;
    use rusqlite::params;

    let content = ureq::get(url)
        .set("Accept", "text/calendar")
        .call()
        .map_err(|e| format!("iCal fetch: {e}"))?
        .into_string()
        .map_err(|e| format!("iCal read: {e}"))?;

    let conn = state.db.lock().map_err(|_| "DB lock failed".to_string())?;
    let _ = conn.execute(
        "INSERT OR REPLACE INTO calendar_ical_cache (url, content, fetched_at) VALUES (?1, ?2, ?3)",
        params![url, &content, now()],
    );
    Ok(content)
}

/// Fetches each saved iCal URL and returns a diagnostic report focused on today's events.
#[tauri::command]
#[specta::specta]
pub fn debug_ical(state: State<'_, AppState>) -> Vec<IcalDebugInfo> {
    use crate::commands::timeline::ical::parse_ics_datetime_pub;

    let settings = load_settings_ical(&state);
    let today = chrono::Local::now().date_naive();

    settings
        .urls
        .iter()
        .filter(|u| !u.trim().is_empty())
        .map(|url| {
            let body = match fetch_ical_fresh(&state, url.trim()) {
                Ok(b) => b,
                Err(e) => {
                    return IcalDebugInfo {
                        url: url.clone(),
                        error: Some(e),
                        total_events: 0,
                        all_day_skipped: 0,
                        recurring_rrule: 0,
                        today_events: vec![],
                        dtstart_samples: vec![],
                    }
                }
            };

            let reader = BufReader::new(body.as_bytes());
            let parser = IcalParser::new(reader);

            let mut total = 0u32;
            let mut all_day = 0u32;
            let mut rrule = 0u32;
            let mut today_events: Vec<(i64, String)> = vec![];
            let mut dtstart_samples: Vec<String> = vec![];

            for calendar in parser.flatten() {
                for event in &calendar.events {
                    total += 1;

                    let get = |name: &str| -> Option<String> {
                        event
                            .properties
                            .iter()
                            .find(|p| p.name == name)
                            .and_then(|p| p.value.clone())
                    };

                    let has_rrule = event.properties.iter().any(|p| p.name == "RRULE");
                    if has_rrule {
                        rrule += 1;
                    }

                    let dtstart_prop = event.properties.iter().find(|p| p.name == "DTSTART");
                    let dtstart_val = dtstart_prop
                        .and_then(|p| p.value.as_deref())
                        .unwrap_or("")
                        .to_string();

                    let is_all_day = dtstart_prop
                        .and_then(|p| p.params.as_ref())
                        .and_then(|ps| ps.iter().find(|(k, _)| k == "VALUE"))
                        .map(|(_, v)| v.iter().any(|s| s == "DATE"))
                        .unwrap_or(false)
                        || !dtstart_val.contains('T');

                    if is_all_day {
                        all_day += 1;
                        continue;
                    }

                    if dtstart_samples.len() < 5 {
                        let params_str = dtstart_prop
                            .and_then(|p| p.params.as_ref())
                            .map(|ps| {
                                ps.iter()
                                    .map(|(k, v)| format!("{k}={}", v.join(",")))
                                    .collect::<Vec<_>>()
                                    .join(";")
                            })
                            .unwrap_or_default();
                        let raw = if params_str.is_empty() {
                            dtstart_val.clone()
                        } else {
                            format!("{dtstart_val} [{params_str}]")
                        };
                        dtstart_samples.push(raw);
                    }

                    if let Some(dt) = parse_ics_datetime_pub(&dtstart_val) {
                        if dt.date_naive() == today {
                            let title = get("SUMMARY").unwrap_or_else(|| "(no title)".into());
                            let suffix = if has_rrule { " (recurring)" } else { "" };
                            today_events.push((
                                dt.timestamp(),
                                format!("{} — {}{}", dt.format("%H:%M"), title, suffix),
                            ));
                        }
                    }
                }
            }

            today_events.sort_by_key(|(ts, _)| *ts);

            IcalDebugInfo {
                url: format!("{}…", &url[..url.len().min(50)]),
                error: None,
                total_events: total,
                all_day_skipped: all_day,
                recurring_rrule: rrule,
                today_events: today_events.into_iter().map(|(_, s)| s).collect(),
                dtstart_samples,
            }
        })
        .collect()
}
