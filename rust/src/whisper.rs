//! Speech-to-text: on-device whisper.cpp (default) or a remote whisper.cpp HTTP server.

use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, Once};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Loaded model, kept between recordings so only the first transcription pays the load cost.
static CONTEXT: LazyLock<Mutex<Option<(PathBuf, WhisperContext)>>> =
    LazyLock::new(|| Mutex::new(None));
static LOGGING: Once = Once::new();

/// Transcribe mono 16 kHz samples with a local GGML Whisper model.
pub fn transcribe_local(model_path: &Path, samples: &[f32]) -> Result<String, String> {
    if samples.is_empty() {
        return Err("No audio samples to transcribe".to_string());
    }
    // Route whisper.cpp's chatty stdout logging through the (silent) `log` facade.
    LOGGING.call_once(whisper_rs::install_logging_hooks);

    let mut guard = CONTEXT.lock().map_err(|_| "Whisper engine is unavailable")?;
    if guard.as_ref().map(|(p, _)| p.as_path()) != Some(model_path) {
        *guard = None;
        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .map_err(|e| format!("Failed to load speech model: {e}"))?;
        *guard = Some((model_path.to_path_buf(), ctx));
    }
    let (_, ctx) = guard.as_ref().unwrap();

    let mut state = ctx
        .create_state()
        .map_err(|e| format!("Failed to start speech model: {e}"))?;

    let threads = std::thread::available_parallelism()
        .map(|n| n.get().min(8))
        .unwrap_or(4) as i32;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(threads);
    params.set_language(Some("en"));
    params.set_no_context(true);
    params.set_no_timestamps(true);
    params.set_suppress_blank(true);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);

    state
        .full(params, samples)
        .map_err(|e| format!("Transcription failed: {e}"))?;

    let text: String = state
        .as_iter()
        .filter_map(|segment| segment.to_str_lossy().ok().map(|s| s.into_owned()))
        .collect::<Vec<_>>()
        .join(" ");
    Ok(text.split_whitespace().collect::<Vec<_>>().join(" "))
}

#[derive(Debug, Deserialize)]
struct WhisperJsonResponse {
    text: Option<String>,
}

fn samples_to_wav_bytes(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>, String> {
    let mut cursor = Cursor::new(Vec::new());
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)
            .map_err(|e| format!("Failed to create WAV writer: {}", e))?;
        for sample in samples {
            let pcm = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer
                .write_sample(pcm)
                .map_err(|e| format!("Failed to write WAV sample: {}", e))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("Failed to finalize WAV data: {}", e))?;
    }

    Ok(cursor.into_inner())
}

/// Transcribe mono 16 kHz samples using a whisper.cpp HTTP server (`/inference` endpoint).
pub fn transcribe_remote(url: &str, samples: &[f32]) -> Result<String, String> {
    if samples.is_empty() {
        return Err("No audio samples to transcribe".to_string());
    }
    if url.trim().is_empty() {
        return Err("No Whisper server URL set. Add one in Settings.".to_string());
    }

    let wav = samples_to_wav_bytes(samples, 16_000)?;
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to start Whisper HTTP runtime: {}", e))?;

    rt.block_on(async move {
        let file_part = Part::bytes(wav)
            .file_name("recording.wav")
            .mime_str("audio/wav")
            .map_err(|e| format!("Failed to prepare audio upload: {}", e))?;

        let form = Form::new()
            .part("file", file_part)
            .text("response_format", "json")
            .text("temperature", "0");

        let response = reqwest::Client::new()
            .post(url)
            .multipart(form)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| format!("Failed to reach Whisper server at {}: {}", url, e))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| format!("Failed to read Whisper response: {}", e))?;

        if !status.is_success() {
            return Err(format!("Whisper server returned HTTP {}: {}", status, body));
        }

        if let Ok(json) = serde_json::from_str::<WhisperJsonResponse>(&body) {
            if let Some(text) = json.text {
                return Ok(text.trim().to_string());
            }
        }
        Ok(body.trim().to_string())
    })
}
