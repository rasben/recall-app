use rusqlite::params;
use std::sync::{Arc, Mutex};

const PING_KEY: &str = "telemetry_last_ping";
const PING_URL: &str = "https://rasben.github.io/recall-app/telemetry-ping.html";

/// Fire a once-per-day anonymous ping to the GitHub Pages telemetry-ping.html.
/// This lets us see rough active-user counts via GitHub's Insights → Traffic.
/// No personal data is sent — it's a plain GET request, identical to a browser visit.
pub fn maybe_ping(db: Arc<Mutex<rusqlite::Connection>>) {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    // Check when we last pinged
    let last_ping: Option<String> = {
        let conn = match db.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut stmt = match conn.prepare("SELECT value FROM settings WHERE key = ?1") {
            Ok(s) => s,
            Err(_) => return,
        };
        stmt.query_row(params![PING_KEY], |row| row.get::<_, String>(0))
            .ok()
    };

    if last_ping.as_deref() == Some(today.as_str()) {
        return; // Already pinged today
    }

    // Persist today's date before pinging so we don't double-ping on restart
    {
        let conn = match db.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let _ = conn.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![PING_KEY, today, now_ms],
        );
    }

    // Fire the ping in the background — blocking HTTP call wrapped in spawn_blocking
    tauri::async_runtime::spawn(async {
        let _ = tauri::async_runtime::spawn_blocking(|| {
            let _ = ureq::get(PING_URL).call();
        })
        .await;
    });
}
