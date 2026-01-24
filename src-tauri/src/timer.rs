use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

static TIMER_START: Mutex<Option<Instant>> = Mutex::new(None);
static TIMER_DURATION: Mutex<Option<Duration>> = Mutex::new(None);

pub fn setup_awareness_timer(app: AppHandle) {
    // Load timer duration from config
    load_timer_duration(&app);
    
    // Initialize timer
    reset_timer().unwrap();
    
    // Listen for Windows session unlock events
    // This is a simplified version - in production, you'd use WTSSESSION_UNLOCK
    tauri::async_runtime::spawn(async move {
        loop {
            // Check every 10 seconds to minimize CPU usage
            tokio::time::sleep(Duration::from_secs(10)).await;

            if let Ok(remaining) = get_remaining_time() {
                if remaining == 0 {
                    // Timer expired - trigger alert
                    trigger_alert(&app);
                    reset_timer().unwrap();
                }
            }
        }
    });
}

fn get_timer_duration() -> Duration {
    let duration = TIMER_DURATION.lock().unwrap();
    duration.unwrap_or_else(|| Duration::from_secs(15 * 60)) // Default 15 minutes
}

pub fn set_timer_duration(app: &AppHandle, minutes: u64) -> Result<(), String> {
    let duration = Duration::from_secs(minutes * 60);
    {
        let mut timer_duration = TIMER_DURATION.lock().map_err(|e| e.to_string())?;
        *timer_duration = Some(duration);
    }
    
    // Save to config file
    save_timer_duration(app, minutes)?;
    
    // Reset timer with new duration
    reset_timer()?;
    
    Ok(())
}

pub fn get_timer_duration_minutes() -> Result<u64, String> {
    let duration = TIMER_DURATION.lock().map_err(|e| e.to_string())?;
    Ok(duration.unwrap_or_else(|| Duration::from_secs(15 * 60)).as_secs() / 60)
}

fn load_timer_duration(app: &AppHandle) {
    if let Ok(app_data_dir) = app.path().app_data_dir() {
        let config_path = app_data_dir.join("timer_config.json");
        
        if let Ok(json) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str::<serde_json::Value>(&json) {
                if let Some(minutes) = config.get("duration_minutes").and_then(|v| v.as_u64()) {
                    let duration = Duration::from_secs(minutes * 60);
                    if let Ok(mut timer_duration) = TIMER_DURATION.lock() {
                        *timer_duration = Some(duration);
                    }
                }
            }
        }
    }
}

fn save_timer_duration(app: &AppHandle, minutes: u64) -> Result<(), String> {
    let app_data_dir = app.path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;
    
    std::fs::create_dir_all(&app_data_dir)
        .map_err(|e| format!("Failed to create app data directory: {}", e))?;
    
    let config_path = app_data_dir.join("timer_config.json");
    let config = serde_json::json!({
        "duration_minutes": minutes
    });
    
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize timer config: {}", e))?;
    
    std::fs::write(&config_path, json)
        .map_err(|e| format!("Failed to write timer config: {}", e))?;
    
    Ok(())
}

pub fn get_remaining_time() -> Result<u64, String> {
    let start = TIMER_START.lock().map_err(|e| e.to_string())?;
    let duration = get_timer_duration();
    
    if let Some(start_time) = *start {
        let elapsed = start_time.elapsed();
        if elapsed >= duration {
            Ok(0)
        } else {
            Ok((duration - elapsed).as_secs())
        }
    } else {
        Ok(duration.as_secs())
    }
}

pub fn reset_timer() -> Result<(), String> {
    let mut start = TIMER_START.lock().map_err(|e| e.to_string())?;
    *start = Some(Instant::now());
    Ok(())
}

fn trigger_alert(app: &AppHandle) {
    // Emit event to frontend
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("timer-alert", ());

        // Visual pulse - flash window
        let _ = window.show();
        let _ = window.set_focus();
    }

    // TODO: Play chime sound
    // For now, we'll rely on frontend to handle audio
}
