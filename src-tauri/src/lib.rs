mod commands;
mod core;
mod state;

#[allow(unused_imports)]
use tauri::Manager as _;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::session::generate_identity,
            commands::session::connect_i2p,
            commands::session::initiate_session,
            commands::session::close_session,
            commands::session::panic_wipe,
            commands::session::update_settings,
            commands::session::get_settings,
            commands::messaging::send_message,
            commands::messaging::get_messages,
        ])
        .setup(|app| {
            // Start TTL background wiper
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(core::ttl::ttl_wiper(handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running ech0");
}
