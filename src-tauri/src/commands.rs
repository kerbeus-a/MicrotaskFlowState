use serde::{Deserialize, Serialize};
use tauri::{State, AppHandle, Manager, Window, Emitter};
use crate::database::Database;
use crate::whisper::{WhisperModelSize, WhisperCache, download_model, check_model_exists, delete_model, transcribe_with_context};

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskResponse {
    pub id: i64,
    pub text: String,
    pub completed: bool,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[tauri::command]
pub fn get_tasks(db: State<Database>) -> Result<Vec<TaskResponse>, String> {
    crate::database::get_all_tasks(&db)
        .map_err(|e: rusqlite::Error| e.to_string())
        .map(|tasks: Vec<crate::database::Task>| {
            tasks.into_iter().map(|t| TaskResponse {
                id: t.id,
                text: t.text,
                completed: t.completed,
                created_at: t.created_at,
                completed_at: t.completed_at,
            }).collect()
        })
}

#[tauri::command]
pub fn add_task(text: String, db: State<Database>) -> Result<TaskResponse, String> {
    crate::database::add_task(&db, &text)
        .map_err(|e: rusqlite::Error| e.to_string())
        .map(|task: crate::database::Task| TaskResponse {
            id: task.id,
            text: task.text,
            completed: task.completed,
            created_at: task.created_at,
            completed_at: task.completed_at,
        })
}

#[tauri::command]
pub fn update_task(id: i64, text: String, db: State<Database>) -> Result<(), String> {
    crate::database::update_task(&db, id, &text)
        .map_err(|e: rusqlite::Error| e.to_string())
}

#[tauri::command]
pub fn delete_task(id: i64, db: State<Database>) -> Result<(), String> {
    crate::database::delete_task(&db, id)
        .map_err(|e: rusqlite::Error| e.to_string())
}

#[tauri::command]
pub fn toggle_task(id: i64, db: State<Database>) -> Result<TaskResponse, String> {
    crate::database::toggle_task(&db, id)
        .map_err(|e: rusqlite::Error| e.to_string())
        .map(|task: crate::database::Task| TaskResponse {
            id: task.id,
            text: task.text,
            completed: task.completed,
            created_at: task.created_at,
            completed_at: task.completed_at,
        })
}

#[tauri::command]
pub async fn process_voice_log(transcript: String, db: State<'_, Database>) -> Result<Vec<TaskResponse>, String> {
    // Use local LLM to parse transcript
    let parsed_tasks: Vec<crate::database::Task> = crate::ollama::parse_transcript(&transcript).await
        .map_err(|e| format!("Failed to parse transcript: {}", e))?;
    
    // Update database with parsed tasks
    let mut results = Vec::new();
    for task in parsed_tasks {
        if task.completed {
            // Mark existing task as completed or create new one
            if let Ok(existing) = crate::database::find_and_complete_task(&db, &task.text) {
                results.push(TaskResponse {
                    id: existing.id,
                    text: existing.text,
                    completed: existing.completed,
                    created_at: existing.created_at,
                    completed_at: existing.completed_at,
                });
            }
        } else {
            // Add new task
            if let Ok(new_task) = crate::database::add_task(&db, &task.text) {
                results.push(TaskResponse {
                    id: new_task.id,
                    text: new_task.text,
                    completed: new_task.completed,
                    created_at: new_task.created_at,
                    completed_at: new_task.completed_at,
                });
            }
        }
    }
    
    Ok(results)
}

#[tauri::command]
pub fn get_timer_status() -> Result<u64, String> {
    crate::timer::get_remaining_time()
        .map_err(|e: String| e)
}

#[tauri::command]
pub fn reset_timer() -> Result<(), String> {
    crate::timer::reset_timer()
        .map_err(|e: String| e)
}

#[tauri::command]
pub fn set_always_on_top(window: Window, always_on_top: bool) -> Result<(), String> {
    window.set_always_on_top(always_on_top)
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub filename: String,
    pub size_mb: u64,
    pub installed: bool,
}

#[tauri::command]
pub fn list_whisper_models(app: AppHandle) -> Result<Vec<ModelInfo>, String> {
    // This command needs AppHandle to access app data directory
    let models = vec![
        (WhisperModelSize::Tiny, "Tiny"),
        (WhisperModelSize::Base, "Base"),
        (WhisperModelSize::Small, "Small"),
        (WhisperModelSize::Medium, "Medium"),
        (WhisperModelSize::Large, "Large"),
    ];

    Ok(models.into_iter().map(|(size, name)| {
        ModelInfo {
            name: name.to_string(),
            filename: size.filename().to_string(),
            size_mb: size.size_mb(),
            installed: check_model_exists(&app, size),
        }
    }).collect())
}

#[tauri::command]
pub async fn download_whisper_model(
    app: AppHandle,
    model_name: String,
) -> Result<String, String> {
    let model_size = WhisperModelSize::from_str(&model_name)
        .ok_or_else(|| format!("Invalid model name: {}", model_name))?;

    // Emit progress events
    let app_handle = app.clone();
    let progress_callback = Box::new(move |downloaded: u64, total: u64| {
        if let Some(window) = app_handle.get_webview_window("main") {
            let progress = if total > 0 {
                (downloaded as f64 / total as f64 * 100.0) as u32
            } else {
                0
            };
            let _ = window.emit("model-download-progress", serde_json::json!({
                "model": model_name,
                "downloaded": downloaded,
                "total": total,
                "progress": progress,
            }));
        }
    });

    let path = download_model(&app, model_size, Some(progress_callback)).await?;
    
    Ok(format!("Model downloaded successfully to: {}", path.to_string_lossy()))
}

#[tauri::command]
pub fn check_whisper_model(app: AppHandle, model_name: String) -> Result<bool, String> {
    let model_size = WhisperModelSize::from_str(&model_name)
        .ok_or_else(|| format!("Invalid model name: {}", model_name))?;
    
    Ok(check_model_exists(&app, model_size))
}

#[tauri::command]
pub fn delete_whisper_model(
    app: AppHandle,
    model_name: String,
    whisper_cache: State<'_, WhisperCache>,
) -> Result<(), String> {
    let model_size = WhisperModelSize::from_str(&model_name)
        .ok_or_else(|| format!("Invalid model name: {}", model_name))?;

    // Clear the cache to avoid using stale model reference
    whisper_cache.clear();

    delete_model(&app, model_size)
}

#[tauri::command]
pub async fn transcribe_audio(
    app: AppHandle,
    audio_path: String,
    model_name: String,
    whisper_cache: State<'_, WhisperCache>,
) -> Result<String, String> {
    let model_size = WhisperModelSize::from_str(&model_name)
        .ok_or_else(|| format!("Invalid model name: {}", model_name))?;

    // Get cached Whisper context
    let ctx = whisper_cache.get_or_create(&app, model_size)?;
    transcribe_with_context(&ctx, &audio_path)
}

#[tauri::command]
pub async fn save_audio_file(
    app: AppHandle,
    audio_data: Vec<u8>,
) -> Result<String, String> {
    use std::io::Write;

    let app_data_dir = app.path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    let audio_temp_dir = app_data_dir.join("audio_temp");
    std::fs::create_dir_all(&audio_temp_dir)
        .map_err(|e| format!("Failed to create audio temp directory: {}", e))?;

    // Generate unique filename with timestamp
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%f");
    let filename = format!("recording_{}.wav", timestamp);
    let file_path = audio_temp_dir.join(&filename);

    // Write audio data to file
    let mut file = std::fs::File::create(&file_path)
        .map_err(|e| format!("Failed to create audio file: {}", e))?;

    file.write_all(&audio_data)
        .map_err(|e| format!("Failed to write audio data: {}", e))?;

    Ok(file_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn process_voice_recording(
    app: AppHandle,
    audio_data: Vec<u8>,
    model_name: String,
    db: State<'_, Database>,
    whisper_cache: State<'_, WhisperCache>,
) -> Result<Vec<TaskResponse>, String> {
    // Save audio to temporary file
    let audio_path = save_audio_file(app.clone(), audio_data).await?;

    // Ensure we have a model
    let model_size = WhisperModelSize::from_str(&model_name)
        .ok_or_else(|| format!("Invalid model name: {}", model_name))?;

    // Get cached Whisper context (avoids reloading model on every recording)
    let ctx = whisper_cache.get_or_create(&app, model_size)?;

    // Transcribe audio using cached context
    let transcript = transcribe_with_context(&ctx, &audio_path)
        .map_err(|e| {
            // Clean up temp file even on error
            let _ = std::fs::remove_file(&audio_path);
            e
        })?;

    // Clean up temp file after successful transcription
    let _ = std::fs::remove_file(&audio_path);

    eprintln!("🎤 Transcription complete: \"{}\"", transcript);

    // First, handle removal actions using simple parser (fast, no network)
    // Only use Ollama for removal if simple parser detects removal keywords
    let transcript_lower = transcript.to_lowercase();
    let has_removal_keywords = ["delete", "remove", "cancel", "drop", "forget", "scratch", "erase"]
        .iter()
        .any(|kw| transcript_lower.contains(kw));

    if has_removal_keywords {
        eprintln!("🔍 Checking for removal actions...");
        let removal_texts = crate::ollama::get_removal_actions(&transcript);
        for removal_text in removal_texts {
            if let Ok(Some(deleted_task)) = crate::database::find_and_delete_task(&db, &removal_text) {
                eprintln!("🗑️ Deleted task: {}", deleted_task.text);
            }
        }
    }

    // Parse transcript for add/complete actions
    eprintln!("📝 Parsing transcript for tasks...");
    let parsed_tasks = crate::ollama::parse_transcript(&transcript).await
        .map_err(|e| format!("Failed to parse transcript: {}", e))?;
    eprintln!("✅ Found {} tasks", parsed_tasks.len());

    // Update database with parsed tasks
    let mut results = Vec::new();
    for task in parsed_tasks {
        if task.completed {
            // Mark existing task as completed or create new one
            if let Ok(existing) = crate::database::find_and_complete_task(&db, &task.text) {
                results.push(TaskResponse {
                    id: existing.id,
                    text: existing.text,
                    completed: existing.completed,
                    created_at: existing.created_at,
                    completed_at: existing.completed_at,
                });
            }
        } else {
            // Add new task
            if let Ok(new_task) = crate::database::add_task(&db, &task.text) {
                results.push(TaskResponse {
                    id: new_task.id,
                    text: new_task.text,
                    completed: new_task.completed,
                    created_at: new_task.created_at,
                    completed_at: new_task.completed_at,
                });
            }
        }
    }

    Ok(results)
}

// Windows auto-start functionality
#[cfg(target_os = "windows")]
mod autostart {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegOpenKeyExW, RegSetValueExW, RegDeleteValueW, RegQueryValueExW, RegCloseKey,
        HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, HKEY,
    };
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    const APP_NAME: &str = "MicroTask";
    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    fn to_wide_null(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    pub fn set_autostart(enabled: bool, exe_path: &str) -> Result<(), String> {
        unsafe {
            let key_path = to_wide_null(RUN_KEY);
            let mut hkey: HKEY = HKEY::default();

            let result = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(key_path.as_ptr()),
                0,
                KEY_WRITE,
                &mut hkey,
            );

            if result.is_err() {
                return Err(format!("Failed to open registry key: {:?}", result));
            }

            let app_name_wide = to_wide_null(APP_NAME);

            let result = if enabled {
                let exe_path_wide = to_wide_null(exe_path);
                let bytes: &[u8] = std::slice::from_raw_parts(
                    exe_path_wide.as_ptr() as *const u8,
                    exe_path_wide.len() * 2,
                );
                RegSetValueExW(
                    hkey,
                    PCWSTR(app_name_wide.as_ptr()),
                    0,
                    REG_SZ,
                    Some(bytes),
                )
            } else {
                RegDeleteValueW(hkey, PCWSTR(app_name_wide.as_ptr()))
            };

            let _ = RegCloseKey(hkey);

            if result.is_err() {
                // Ignore error when deleting non-existent value
                if !enabled {
                    return Ok(());
                }
                return Err(format!("Failed to modify registry: {:?}", result));
            }

            Ok(())
        }
    }

    pub fn get_autostart() -> Result<bool, String> {
        unsafe {
            let key_path = to_wide_null(RUN_KEY);
            let mut hkey: HKEY = HKEY::default();

            let result = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(key_path.as_ptr()),
                0,
                KEY_READ,
                &mut hkey,
            );

            if result.is_err() {
                return Ok(false);
            }

            let app_name_wide = to_wide_null(APP_NAME);
            let mut data_size: u32 = 0;

            let result = RegQueryValueExW(
                hkey,
                PCWSTR(app_name_wide.as_ptr()),
                None,
                None,
                None,
                Some(&mut data_size),
            );

            let _ = RegCloseKey(hkey);

            Ok(result.is_ok() && data_size > 0)
        }
    }
}

#[tauri::command]
pub fn get_autostart_enabled() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        autostart::get_autostart()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}

#[tauri::command]
pub fn set_autostart_enabled(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let exe_path = std::env::current_exe()
            .map_err(|e| format!("Failed to get executable path: {}", e))?
            .to_string_lossy()
            .to_string();
        autostart::set_autostart(enabled, &exe_path)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Err("Auto-start is only supported on Windows".to_string())
    }
}
