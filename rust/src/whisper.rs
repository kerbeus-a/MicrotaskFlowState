use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::io::Cursor;

const DEFAULT_WHISPER_URL: &str = "http://192.168.1.14:8080/inference";

#[derive(Debug, Deserialize)]
struct WhisperJsonResponse {
    text: Option<String>,
}

fn whisper_url() -> String {
    std::env::var("WHISPER_URL").unwrap_or_else(|_| DEFAULT_WHISPER_URL.to_string())
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
            let clamped = sample.clamp(-1.0, 1.0);
            let pcm = (clamped * i16::MAX as f32) as i16;
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

/// Transcribe raw mono 16 kHz audio samples using a remote whisper.cpp HTTP server.
///
/// Default endpoint: http://192.168.1.14:8080/inference
/// Override with WHISPER_URL if the server uses another address or route.
pub fn transcribe_audio(samples: &[f32]) -> Result<String, String> {
    if samples.is_empty() {
        return Err("No audio samples to transcribe".to_string());
    }

    let wav = samples_to_wav_bytes(samples, 16_000)?;
    let url = whisper_url();
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
            .post(&url)
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
                let text = text.trim().to_string();
                eprintln!("✅ Remote Whisper transcribed: {}", text);
                return Ok(text);
            }
        }

        let text = body.trim().to_string();
        eprintln!("✅ Remote Whisper transcribed: {}", text);
        Ok(text)
    })
}
