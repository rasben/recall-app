mod commands;
mod db;
mod state;
mod timeline;

use state::AppState;
use std::sync::Mutex;
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
        commands::timeline::get_timeline_for_day,
        commands::harvest_done::get_timeline_harvest_done_for_event_ids,
        commands::harvest_done::set_timeline_harvest_done,
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
            let conn = db::init_db(app.handle());

            app.manage(AppState {
                db: Mutex::new(conn),
            });

            builder.mount_events(app);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
