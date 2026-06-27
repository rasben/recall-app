mod commands;
mod db;
mod state;
mod telemetry;
mod timeline;

use state::AppState;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use tauri::Manager;
use tauri_specta::Builder;

/// Builder with every command registered. Extracted so tests can exercise the
/// specta export the same way the running app does.
fn make_specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        commands::settings_ui::set_settings_ui,
        commands::settings_ui::get_settings_ui,
        commands::settings_export::set_settings_export,
        commands::settings_export::get_settings_export,
        commands::settings_git::set_settings_git,
        commands::settings_git::get_settings_git,
        commands::settings_github::set_settings_github,
        commands::settings_github::get_settings_github,
        commands::settings_jira::set_settings_jira,
        commands::settings_jira::get_settings_jira,
        commands::settings_zulip::set_settings_zulip,
        commands::settings_zulip::get_settings_zulip,
        commands::settings_ical::set_settings_ical,
        commands::settings_ical::get_settings_ical,
        commands::settings_ical::trigger_ical_sync,
        commands::settings_ical::get_ical_sync_status,
        commands::timeline::get_timeline_for_day,
        commands::timeline::refresh_timeline_for_day,
        commands::timeline::export_timeline_for_range,
        commands::timeline::get_day_counts_for_month,
        commands::timeline::test_settings_git,
        commands::timeline::test_settings_github,
        commands::timeline::test_settings_jira,
        commands::timeline::test_settings_zulip,
        commands::timeline::test_settings_ical,
        commands::harvest_done::get_timeline_harvest_done_for_event_ids,
        commands::harvest_done::set_timeline_harvest_done,
        commands::settings::clear_all_caches,
        commands::settings::get_cache_size,
        commands::settings::get_cached_day_event_counts,
    ])
}

/// Specta Typescript config used by the export. Map Rust's i64 (e.g.
/// TimelineEvent.timestamp = unix seconds) to TS `number`; unix seconds fit
/// far inside Number.MAX_SAFE_INTEGER, so the default BigIntForbidden policy
/// is unnecessarily strict.
fn specta_typescript_config() -> specta_typescript::Typescript {
    specta_typescript::Typescript::default()
        .bigint(specta_typescript::BigIntExportBehavior::Number)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = make_specta_builder();

    #[cfg(debug_assertions)]
    builder
        .export(specta_typescript_config(), "../src/bindings.ts")
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            let (conn, db_path) = db::init_db(app.handle())?;

            app.manage(AppState {
                db: Arc::new(Mutex::new(conn)),
                db_path: db_path.clone(),
                ical_syncing: Arc::new(AtomicBool::new(false)),
            });

            telemetry::spawn_ping(db_path);

            builder.mount_events(app);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real specta export only runs when the app boots (in `run()`), which
    /// `cargo test` never does — so issues like `BigIntForbidden` on a new
    /// `i64` field silently slipped through until a developer ran the app.
    /// Run it from the test suite too, writing to the same `src/bindings.ts`
    /// file the dev build would; this doubles as a regen path so tests refresh
    /// bindings if any command signature changed.
    #[test]
    fn typescript_bindings_export_successfully() {
        // CARGO_MANIFEST_DIR is src-tauri/; bindings live one level up under src/.
        let bindings_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../src/bindings.ts");
        make_specta_builder()
            .export(specta_typescript_config(), &bindings_path)
            .expect("typescript bindings must export without errors");
    }
}
