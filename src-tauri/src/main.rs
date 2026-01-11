// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod database;
mod timer;
mod whisper;
mod ollama;

use tauri::{Manager, WindowEvent};

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Initialize database
            let db = database::init_database(app)?;
            app.manage(db);
            
            // Setup global shortcut (Win + Alt + R)
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = setup_global_shortcut(&app_handle).await {
                    eprintln!("Failed to setup global shortcut: {}", e);
                }
            });
            
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
        ])
        .on_window_event(|event| {
            if let WindowEvent::CloseRequested { api, .. } = event.event() {
                // Hide window instead of closing
                event.window().hide().unwrap();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn setup_global_shortcut(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::GlobalShortcutManager;
    
    let mut shortcut_manager = app.global_shortcut();
    
    // Register Win + Alt + R
    shortcut_manager.register("Alt+Meta+R", move || {
        if let Some(window) = app.get_window("main") {
            window.show().unwrap();
            window.set_focus().unwrap();
            // Trigger recording (this will be handled by frontend)
            window.emit("start-recording", ()).unwrap();
        }
    })?;
    
    Ok(())
}
