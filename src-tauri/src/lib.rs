mod commands;
mod db;
mod state;

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
        ]);

    #[cfg(debug_assertions)]
    builder
        .export(specta_typescript::Typescript::default(), "../src/bindings.ts")
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
