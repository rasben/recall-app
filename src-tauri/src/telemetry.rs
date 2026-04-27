/// Privacy-first install counter.
///
/// On startup the app fires a single background ping to a Cloudflare Worker.
/// The ping carries only a random UUID that is generated once and stored
/// locally — no personal data, no device info, no IP is persisted server-side.
///
/// Rate-limited to once per calendar day so it does not spam the worker.
/// All errors are silently swallowed; telemetry must never affect the user.
///
/// To deploy the worker and fill in the URL, see worker/wrangler.toml.
use chrono::Local;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use uuid::Uuid;

const PING_URL: &str = "https://recall-telemetry.recall-app.workers.dev/ping";

const KEY_INSTALL_ID: &str = "telemetry_install_id";
const KEY_LAST_PING: &str = "telemetry_last_ping_date";

/// Spawn a background thread that pings the telemetry worker (at most once per day).
/// Designed to be called once from `app.setup()` and then forgotten.
pub fn spawn_ping(db_path: PathBuf) {
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = ping(&db_path) {
            // Telemetry failures are intentionally silent in release builds.
            #[cfg(debug_assertions)]
            eprintln!("[telemetry] ping failed: {e}");
            let _ = e; // suppress unused-variable warning in release
        }
    });
}

fn ping(db_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Skip if the URL hasn't been filled in yet (dev / before worker deploy).
    if PING_URL.contains("REPLACE_ME") {
        return Ok(());
    }

    let conn = Connection::open(db_path)?;

    // Rate-limit: only ping once per calendar day.
    let today = Local::now().format("%Y-%m-%d").to_string();
    if let Ok(last) = conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![KEY_LAST_PING],
        |row| row.get::<_, String>(0),
    ) {
        if last == today {
            return Ok(()); // already pinged today
        }
    }

    // Get or create a stable, random install ID.
    let install_id = match conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![KEY_INSTALL_ID],
        |row| row.get::<_, String>(0),
    ) {
        Ok(id) => id,
        Err(_) => {
            let id = Uuid::new_v4().to_string();
            let ts = now_ms();
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                params![KEY_INSTALL_ID, id, ts],
            )?;
            id
        }
    };

    // Fire the ping. ureq 3.x returns Err for non-2xx, so `?` handles the
    // error path; if we reach the next line the request succeeded.
    let body = serde_json::json!({ "id": install_id }).to_string();
    ureq::post(PING_URL)
        .header("Content-Type", "application/json")
        .send(body)?;

    // Record today so we don't ping again until tomorrow.
    let ts = now_ms();
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
        params![KEY_LAST_PING, today, ts],
    )?;

    Ok(())
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
