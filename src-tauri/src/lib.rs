mod commands;
mod db;
mod state;
mod timeline;

use state::AppState;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use tauri::Manager;
use tauri_specta::Builder;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        commands::settings_ui::set_settings_ui,
        commands::settings_ui::get_settings_ui,
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
        commands::harvest_done::get_timeline_harvest_done_for_event_ids,
        commands::harvest_done::set_timeline_harvest_done,
        commands::settings::clear_all_caches,
        commands::settings::get_cache_size,
        commands::settings::get_cached_day_event_counts,
    ]);

    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default(),
            "../src/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            let (conn, db_path) = db::init_db(app.handle())?;

            app.manage(AppState {
                db: Arc::new(Mutex::new(conn)),
                db_path,
                ical_syncing: Arc::new(AtomicBool::new(false)),
            });

            builder.mount_events(app);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
