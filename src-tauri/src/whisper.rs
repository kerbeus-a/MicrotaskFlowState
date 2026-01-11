// Whisper.cpp integration module
// This will handle local speech-to-text conversion and model management

use std::path::PathBuf;
use std::fs;
use tauri::AppHandle;
use serde::{Deserialize, Serialize};

pub struct WhisperEngine {
    model_path: PathBuf,
    model_size: WhisperModelSize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WhisperModelSize {
    Tiny,   // ~75 MB
    Base,   // ~142 MB
    Small,  // ~466 MB
    Medium, // ~1.4 GB
    Large,  // ~2.9 GB
}

impl WhisperModelSize {
    pub fn filename(&self) -> &'static str {
        match self {
            WhisperModelSize::Tiny => "ggml-tiny.bin",
            WhisperModelSize::Base => "ggml-base.bin",
            WhisperModelSize::Small => "ggml-small.bin",
            WhisperModelSize::Medium => "ggml-medium.bin",
            WhisperModelSize::Large => "ggml-large-v3.bin",
        }
    }

    pub fn url(&self) -> &'static str {
        // Using Hugging Face CDN for Whisper models
        match self {
            WhisperModelSize::Tiny => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
            WhisperModelSize::Base => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
            WhisperModelSize::Small => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
            WhisperModelSize::Medium => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
            WhisperModelSize::Large => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
        }
    }

    pub fn size_mb(&self) -> u64 {
        match self {
            WhisperModelSize::Tiny => 75,
            WhisperModelSize::Base => 142,
            WhisperModelSize::Small => 466,
            WhisperModelSize::Medium => 1400,
            WhisperModelSize::Large => 2900,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "tiny" => Some(WhisperModelSize::Tiny),
            "base" => Some(WhisperModelSize::Base),
            "small" => Some(WhisperModelSize::Small),
            "medium" => Some(WhisperModelSize::Medium),
            "large" => Some(WhisperModelSize::Large),
            _ => None,
        }
    }
}

impl WhisperEngine {
    pub fn get_models_dir(app: &AppHandle) -> Result<PathBuf, String> {
        let app_data_dir = app.path_resolver()
            .app_data_dir()
            .ok_or("Failed to get app data directory")?;
        
        let models_dir = app_data_dir.join("whisper_models");
        std::fs::create_dir_all(&models_dir)
            .map_err(|e| format!("Failed to create models directory: {}", e))?;
        
        Ok(models_dir)
    }

    pub fn get_model_path(app: &AppHandle, model_size: WhisperModelSize) -> PathBuf {
        let models_dir = Self::get_models_dir(app).unwrap_or_else(|_| {
            std::env::temp_dir().join("flowstate_models")
        });
        models_dir.join(model_size.filename())
    }

    pub fn new(app: &AppHandle, model_size: WhisperModelSize) -> Result<Self, String> {
        let model_path = Self::get_model_path(app, model_size);
        
        if !model_path.exists() {
            return Err(format!(
                "Model {} not found. Please download it first.",
                model_size.filename()
            ));
        }

        Ok(Self {
            model_path,
            model_size,
        })
    }

    pub fn transcribe(&self, audio_path: &str) -> Result<String, String> {
        // This is a placeholder - actual implementation will call whisper.cpp
        // For now, we'll need to integrate with whisper.cpp binary or library
        
        // Example command (adjust based on your whisper.cpp setup):
        // whisper.cpp/main -m models/ggml-base.bin -f audio.wav
        
        // TODO: Implement actual whisper.cpp integration
        // For development, you can use a mock or call the binary
        
        // Check if whisper.cpp binary exists
        let whisper_binary = self.find_whisper_binary();
        
        if whisper_binary.is_none() {
            return Err("Whisper.cpp binary not found. Please install whisper.cpp and add it to PATH.".to_string());
        }

        // For now, return a placeholder
        Err("Whisper transcription not yet fully implemented. Model is ready at: ".to_string() + 
            &self.model_path.to_string_lossy())
    }

    fn find_whisper_binary(&self) -> Option<PathBuf> {
        // Try to find whisper.cpp binary
        // First check if it's in PATH
        if let Ok(path) = std::env::var("PATH") {
            for dir in path.split(std::path::MAIN_SEPARATOR) {
                let binary_path = PathBuf::from(dir).join("whisper");
                if binary_path.exists() {
                    return Some(binary_path);
                }
            }
        }
        
        // Check common installation locations
        let common_paths = vec![
            PathBuf::from("whisper"),
            PathBuf::from("./whisper"),
            PathBuf::from("../whisper.cpp/main"),
        ];
        
        for path in common_paths {
            if path.exists() {
                return Some(path);
            }
        }
        
        None
    }
}

// Model download functions
pub async fn download_model(
    app: &AppHandle,
    model_size: WhisperModelSize,
    on_progress: Option<Box<dyn Fn(u64, u64) + Send>>,
) -> Result<PathBuf, String> {
    let models_dir = WhisperEngine::get_models_dir(app)?;
    let model_path = models_dir.join(model_size.filename());
    
    // If model already exists, return it
    if model_path.exists() {
        return Ok(model_path);
    }

    let url = model_size.url();
    let client = reqwest::Client::new();
    
    // Download the model
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to download model: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Failed to download model: HTTP {}", response.status()));
    }

    let total_size = response.content_length().unwrap_or(0);
    let mut file = fs::File::create(&model_path)
        .map_err(|e| format!("Failed to create model file: {}", e))?;
    
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;

    use futures_util::StreamExt;
    use std::io::Write;

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| format!("Download error: {}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("Failed to write to file: {}", e))?;
        
        downloaded += chunk.len() as u64;
        
        if let Some(ref callback) = on_progress {
            callback(downloaded, total_size);
        }
    }

    Ok(model_path)
}

pub fn check_model_exists(app: &AppHandle, model_size: WhisperModelSize) -> bool {
    let model_path = WhisperEngine::get_model_path(app, model_size);
    model_path.exists()
}

pub fn list_available_models(app: &AppHandle) -> Vec<(String, bool, u64)> {
    let models = vec![
        WhisperModelSize::Tiny,
        WhisperModelSize::Base,
        WhisperModelSize::Small,
        WhisperModelSize::Medium,
        WhisperModelSize::Large,
    ];

    models.into_iter().map(|model| {
        let exists = check_model_exists(app, model);
        let size = model.size_mb();
        (model.filename().to_string(), exists, size)
    }).collect()
}

pub fn delete_model(app: &AppHandle, model_size: WhisperModelSize) -> Result<(), String> {
    let model_path = WhisperEngine::get_model_path(app, model_size);
    
    if model_path.exists() {
        fs::remove_file(&model_path)
            .map_err(|e| format!("Failed to delete model: {}", e))?;
    }
    
    Ok(())
}

// Helper function to convert audio buffer to WAV file
pub fn save_audio_buffer(buffer: &[u8], output_path: &str) -> Result<(), String> {
    std::fs::write(output_path, buffer)
        .map_err(|e| format!("Failed to save audio: {}", e))
}
