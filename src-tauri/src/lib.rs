mod commands;
mod core;
mod state;

use crate::core::crypto::IdentityKeys;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // Logging only in debug builds — production has no log files
    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .build(),
        );
    }

    builder
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::session::generate_identity,
            commands::session::connect_i2p,
            commands::session::initiate_session,
            commands::session::close_session,
            commands::session::panic_wipe,
            commands::session::update_settings,
            commands::session::get_settings,
            commands::session::get_router_status,
            commands::messaging::send_message,
            commands::messaging::get_messages,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // Generate a fresh identity immediately so SAM connect never fails due to missing keys
            {
                let state = handle.state::<state::AppState>();
                let keys = IdentityKeys::generate();
                *state.identity.try_lock().expect("identity lock") = Some(keys);
            }

            // TTL background wiper
            tauri::async_runtime::spawn(core::ttl::ttl_wiper(handle.clone()));

            // Start embedded I2P router and auto-connect to SAM
            let handle2 = handle.clone();
            tauri::async_runtime::spawn(async move {
                use tauri::Emitter;

                let data_dir = handle2
                    .path()
                    .app_data_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from(".ech0_data"));

                set_router_status(&handle2, "bootstrapping").await;

                match core::router::start_embedded_router(data_dir).await {
                    Ok(sam_port) => {
                        {
                            let state = handle2.state::<state::AppState>();
                            *state.router_sam_port.lock().await = Some(sam_port);
                        }
                        set_router_status(&handle2, "connecting").await;
                        commands::session::auto_connect_loop(handle2).await;
                    }
                    Err(e) => {
                        log::error!("failed to start embedded I2P router: {}", e);
                        set_router_status(&handle2, "error").await;
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running ech0");
}

/// Update router status in AppState and emit the event.
pub async fn set_router_status(app: &tauri::AppHandle, status: &'static str) {
    use tauri::Emitter;
    let state = app.state::<state::AppState>();
    *state.router_status.lock().await = status.to_string();
    let _ = app.emit("router_status_changed", status);
}
