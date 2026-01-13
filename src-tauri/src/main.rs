// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod database;
mod timer;
mod whisper;
mod ollama;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize database
            let app_handle_for_db = app.handle().clone();
            let db = database::init_database(&app_handle_for_db)?;
            app.manage(db);

            // Initialize Whisper model cache (avoids reloading model on every recording)
            let whisper_cache = whisper::WhisperCache::new();
            app.manage(whisper_cache);

            // Setup global shortcut (Win + Alt + R) - DISABLED
            // let app_handle = app.handle().clone();
            // tauri::async_runtime::spawn(async move {
            //     if let Err(e) = setup_global_shortcut(app_handle).await {
            //         eprintln!("Failed to setup global shortcut: {}", e);
            //     }
            // });

            // Setup awareness timer
            timer::setup_awareness_timer(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_tasks,
            commands::add_task,
            commands::update_task,
            commands::delete_task,
            commands::toggle_task,
            commands::process_voice_log,
            commands::get_timer_status,
            commands::reset_timer,
            commands::set_always_on_top,
            commands::list_whisper_models,
            commands::download_whisper_model,
            commands::check_whisper_model,
            commands::delete_whisper_model,
            commands::transcribe_audio,
            commands::save_audio_file,
            commands::process_voice_recording,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide window instead of closing
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Global shortcut setup - DISABLED
// async fn setup_global_shortcut(app: tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
//     use tauri::{GlobalShortcutManager, Manager};
//     
//     let mut shortcut_manager = app.global_shortcut_manager();
//     
//     // Try different shortcut combinations (Windows may reserve Win+Alt combinations)
//     // Priority: Ctrl+Alt+R (most compatible), then F12, then Super+Shift+R
//     let shortcuts = vec!["Ctrl+Alt+R", "F12", "Super+Shift+R"];
//     
//     let mut registered = false;
//     for shortcut in shortcuts {
//         let app_clone = app.clone();
//         let result = shortcut_manager.register(shortcut, move || {
//             if let Some(window) = app_clone.get_window("main") {
//                 let _ = window.show();
//                 let _ = window.set_focus();
//                 // Trigger recording (this will be handled by frontend)
//                 let _ = window.emit("start-recording", ());
//             }
//         });
//         
//         match result {
//             Ok(_) => {
//                 eprintln!("Successfully registered global shortcut: {}", shortcut);
//                 registered = true;
//                 break;
//             }
//             Err(e) => {
//                 eprintln!("Failed to register shortcut {}: {}. Trying next...", shortcut, e);
//                 // Continue to next shortcut
//             }
//         }
//     }
//     
//     if !registered {
//         eprintln!("Warning: Could not register any global shortcut. The app will still work, but voice recording must be triggered manually from the UI.");
//     }
//     
//     Ok(())
// }
