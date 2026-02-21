mod commands;
mod state;

use tauri::Manager;
use tracing::info;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "speakeasy_client=debug,warn".into()),
        )
        .init();

    info!("Speakeasy Client startet...");

    tauri::Builder::default()
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::connect_to_server,
            commands::disconnect,
            commands::join_channel,
            commands::leave_channel,
            commands::get_audio_devices,
            commands::set_audio_config,
            commands::toggle_mute,
            commands::toggle_deafen,
            commands::get_server_info,
            // Chat-Commands (Phase 4)
            commands::send_message,
            commands::get_message_history,
            commands::edit_message,
            commands::delete_message,
            commands::upload_file,
            commands::download_file,
            commands::list_files,
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            #[cfg(debug_assertions)]
            window.open_devtools();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten der Tauri-Anwendung");
}
