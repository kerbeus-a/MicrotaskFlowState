use serde::{Deserialize, Serialize};
use tauri::{State, AppHandle, Manager};
use crate::database::{Task, Database};
use crate::whisper::{WhisperModelSize, download_model, check_model_exists, list_available_models, delete_model};

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
    database::get_all_tasks(&db)
        .map_err(|e| e.to_string())
        .map(|tasks| {
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
    database::add_task(&db, &text)
        .map_err(|e| e.to_string())
        .map(|task| TaskResponse {
            id: task.id,
            text: task.text,
            completed: task.completed,
            created_at: task.created_at,
            completed_at: task.completed_at,
        })
}

#[tauri::command]
pub fn update_task(id: i64, text: String, db: State<Database>) -> Result<(), String> {
    database::update_task(&db, id, &text)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_task(id: i64, db: State<Database>) -> Result<(), String> {
    database::delete_task(&db, id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_task(id: i64, db: State<Database>) -> Result<TaskResponse, String> {
    database::toggle_task(&db, id)
        .map_err(|e| e.to_string())
        .map(|task| TaskResponse {
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
    let parsed_tasks = ollama::parse_transcript(&transcript).await
        .map_err(|e| format!("Failed to parse transcript: {}", e))?;
    
    // Update database with parsed tasks
    let mut results = Vec::new();
    for task in parsed_tasks {
        if task.completed {
            // Mark existing task as completed or create new one
            if let Ok(existing) = database::find_and_complete_task(&db, &task.text) {
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
            if let Ok(new_task) = database::add_task(&db, &task.text) {
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
    timer::get_remaining_time()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reset_timer() -> Result<(), String> {
    timer::reset_timer()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_always_on_top(window: tauri::Window, always_on_top: bool) -> Result<(), String> {
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
        if let Some(window) = app_handle.get_window("main") {
            let progress = if total > 0 {
                (downloaded as f64 / total as f64 * 100.0) as u32
            } else {
                0
            };
            window.emit("model-download-progress", serde_json::json!({
                "model": model_name,
                "downloaded": downloaded,
                "total": total,
                "progress": progress,
            })).ok();
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
pub fn delete_whisper_model(app: AppHandle, model_name: String) -> Result<(), String> {
    let model_size = WhisperModelSize::from_str(&model_name)
        .ok_or_else(|| format!("Invalid model name: {}", model_name))?;
    
    delete_model(&app, model_size)
}

#[tauri::command]
pub async fn transcribe_audio(
    app: AppHandle,
    audio_path: String,
    model_name: String,
) -> Result<String, String> {
    let model_size = WhisperModelSize::from_str(&model_name)
        .ok_or_else(|| format!("Invalid model name: {}", model_name))?;
    
    let engine = crate::whisper::WhisperEngine::new(&app, model_size)?;
    engine.transcribe(&audio_path)
}
