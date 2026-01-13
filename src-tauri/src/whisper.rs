// Whisper.cpp integration module
// This handles local speech-to-text conversion and model management

use std::path::PathBuf;
use std::fs;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use serde::{Deserialize, Serialize};
use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};

/// Helper functions for Whisper model path management
pub struct WhisperPaths;

/// Thread-safe cache for WhisperPaths to avoid reloading models on every recording
pub struct WhisperCache {
    engine: Mutex<Option<(WhisperModelSize, Arc<WhisperContext>)>>,
}

impl WhisperCache {
    pub fn new() -> Self {
        Self {
            engine: Mutex::new(None),
        }
    }

    /// Get or create a WhisperContext for the given model size
    pub fn get_or_create(&self, app: &AppHandle, model_size: WhisperModelSize) -> Result<Arc<WhisperContext>, String> {
        // Recover from poisoned lock (previous panic) by clearing it
        let mut guard = self.engine.lock().unwrap_or_else(|poisoned| {
            eprintln!("⚠️ Recovering from poisoned lock, clearing cache...");
            let mut guard = poisoned.into_inner();
            *guard = None;
            guard
        });

        // Check if we already have the right model loaded
        if let Some((cached_size, ref ctx)) = *guard {
            if cached_size == model_size {
                eprintln!("✅ Using cached Whisper model");
                return Ok(Arc::clone(ctx));
            }
        }

        // Need to load a new model
        let model_path = WhisperPaths::get_model_path(app, model_size);

        if !model_path.exists() {
            return Err(format!(
                "Model {} not found. Please download it first from Settings.",
                model_size.filename()
            ));
        }

        eprintln!("🔄 Loading Whisper model: {} (this may take a moment...)", model_size.filename());

        let ctx = WhisperContext::new_with_params(
            model_path.to_str().ok_or("Invalid model path")?,
            WhisperContextParameters::default(),
        ).map_err(|e| format!("Failed to load Whisper model: {}", e))?;

        let ctx = Arc::new(ctx);
        *guard = Some((model_size, Arc::clone(&ctx)));

        eprintln!("✅ Whisper model loaded successfully!");
        Ok(ctx)
    }

    /// Clear the cached model (useful when user deletes a model)
    pub fn clear(&self) {
        // Recover from poisoned lock if needed
        match self.engine.lock() {
            Ok(mut guard) => *guard = None,
            Err(poisoned) => *poisoned.into_inner() = None,
        }
        eprintln!("🗑️ Whisper cache cleared");
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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

impl WhisperPaths {
    pub fn get_models_dir(app: &AppHandle) -> Result<PathBuf, String> {
        let app_data_dir = app.path()
            .app_data_dir()
            .map_err(|e| format!("Failed to get app data directory: {}", e))?;

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
}

/// Transcribe audio using a cached WhisperContext (avoids reloading model)
pub fn transcribe_with_context(ctx: &WhisperContext, audio_path: &str) -> Result<String, String> {
    // Read WAV file
    let reader = hound::WavReader::open(audio_path)
        .map_err(|e| format!("Failed to open audio file: {}", e))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;

    // Convert samples to f32
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_value = (1 << (spec.bits_per_sample - 1)) as f32;
            reader.into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max_value)
                .collect()
        }
        hound::SampleFormat::Float => {
            reader.into_samples::<f32>()
                .filter_map(|s| s.ok())
                .collect()
        }
    };

    // Resample to 16kHz if needed (Whisper expects 16kHz)
    let samples = if sample_rate != 16000 {
        resample(&samples, sample_rate as usize, 16000)
    } else {
        samples
    };

    // Create whisper state
    let mut state = ctx.create_state()
        .map_err(|e| format!("Failed to create Whisper state: {}", e))?;

    // Set up parameters
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("ru")); // Russian language
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_single_segment(false);

    // Run transcription
    state.full(params, &samples)
        .map_err(|e| format!("Transcription failed: {}", e))?;

    // Collect results
    let num_segments = state.full_n_segments()
        .map_err(|e| format!("Failed to get segments: {}", e))?;

    let mut transcript = String::new();
    for i in 0..num_segments {
        if let Ok(segment) = state.full_get_segment_text(i) {
            transcript.push_str(&segment);
            transcript.push(' ');
        }
    }

    Ok(transcript.trim().to_string())
}

/// Simple linear resampling
fn resample(samples: &[f32], from_rate: usize, to_rate: usize) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let new_len = (samples.len() as f64 / ratio) as usize;
    let mut resampled = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_idx = i as f64 * ratio;
        let idx = src_idx as usize;
        let frac = src_idx - idx as f64;

        let sample = if idx + 1 < samples.len() {
            samples[idx] * (1.0 - frac as f32) + samples[idx + 1] * frac as f32
        } else if idx < samples.len() {
            samples[idx]
        } else {
            0.0
        };

        resampled.push(sample);
    }

    resampled
}

// Model download functions
pub async fn download_model(
    app: &AppHandle,
    model_size: WhisperModelSize,
    on_progress: Option<Box<dyn Fn(u64, u64) + Send>>,
) -> Result<PathBuf, String> {
    let models_dir = WhisperPaths::get_models_dir(app)?;
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
    let model_path = WhisperPaths::get_model_path(app, model_size);
    model_path.exists()
}

#[allow(dead_code)]
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
    let model_path = WhisperPaths::get_model_path(app, model_size);
    
    if model_path.exists() {
        fs::remove_file(&model_path)
            .map_err(|e| format!("Failed to delete model: {}", e))?;
    }
    
    Ok(())
}

// Helper function to convert audio buffer to WAV file
#[allow(dead_code)]
pub fn save_audio_buffer(buffer: &[u8], output_path: &str) -> Result<(), String> {
    std::fs::write(output_path, buffer)
        .map_err(|e| format!("Failed to save audio: {}", e))
}
