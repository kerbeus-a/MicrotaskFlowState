use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::AppHandle;

static TIMER_START: Mutex<Option<Instant>> = Mutex::new(None);
static TIMER_DURATION: Duration = Duration::from_secs(15 * 60); // 15 minutes

pub fn setup_awareness_timer(app: AppHandle) {
    // Initialize timer
    reset_timer().unwrap();
    
    // Listen for Windows session unlock events
    // This is a simplified version - in production, you'd use WTSSESSION_UNLOCK
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            
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

pub fn get_remaining_time() -> Result<u64, String> {
    let start = TIMER_START.lock().map_err(|e| e.to_string())?;
    if let Some(start_time) = *start {
        let elapsed = start_time.elapsed();
        if elapsed >= TIMER_DURATION {
            Ok(0)
        } else {
            Ok((TIMER_DURATION - elapsed).as_secs())
        }
    } else {
        Ok(TIMER_DURATION.as_secs())
    }
}

pub fn reset_timer() -> Result<(), String> {
    let mut start = TIMER_START.lock().map_err(|e| e.to_string())?;
    *start = Some(Instant::now());
    Ok(())
}

fn trigger_alert(app: &AppHandle) {
    // Emit event to frontend
    if let Some(window) = app.get_window("main") {
        window.emit("timer-alert", ()).unwrap();
        
        // Visual pulse - flash window
        window.show().unwrap();
        window.set_focus().unwrap();
    }
    
    // TODO: Play chime sound
    // For now, we'll rely on frontend to handle audio
}
