mod commands;
mod db;
mod state;
mod timeline;

use tauri_specta::Builder;
use tauri::Manager;
use std::sync::Mutex;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            commands::settings::get_language,
            commands::settings::set_language,
            commands::settings::get_theme,
            commands::settings::set_theme,
            commands::settings::get_git_enabled,
            commands::settings::set_git_enabled,
            commands::settings::get_git_scan_path,
            commands::settings::set_git_scan_path,
            commands::timeline::get_timeline_for_day,
        ]);

    #[cfg(debug_assertions)]
    builder
        .export(specta_typescript::Typescript::default(), "../src/bindings.ts")
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            let conn = db::init_db(app.handle());

            app.manage(AppState {
                db: Mutex::new(conn)
            });

            builder.mount_events(app);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
