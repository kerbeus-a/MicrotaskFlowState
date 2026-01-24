// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Native UI version using egui - 0% CPU when idle
//! Run with: cargo run --bin flowstate-native --features native-ui --no-default-features

mod database;
mod ollama;
mod whisper;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use std::thread;

// Result from background processing
enum ProcessingResult {
    Transcript(String),
    Tasks(Vec<database::Task>),
    Error(String),
    Done,
}

// Download state shared between UI and download thread
#[derive(Clone)]
struct DownloadState {
    is_downloading: Arc<Mutex<bool>>,
    current_model: Arc<Mutex<Option<String>>>,
    progress: Arc<Mutex<f32>>,        // 0.0 to 1.0
    downloaded_mb: Arc<Mutex<f32>>,
    total_mb: Arc<Mutex<f32>>,
    error: Arc<Mutex<Option<String>>>,
    completed: Arc<Mutex<bool>>,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self {
            is_downloading: Arc::new(Mutex::new(false)),
            current_model: Arc::new(Mutex::new(None)),
            progress: Arc::new(Mutex::new(0.0)),
            downloaded_mb: Arc::new(Mutex::new(0.0)),
            total_mb: Arc::new(Mutex::new(0.0)),
            error: Arc::new(Mutex::new(None)),
            completed: Arc::new(Mutex::new(false)),
        }
    }
}

// App state
struct FlowStateApp {
    // Database
    db: database::Database,

    // Tasks
    tasks: Vec<database::Task>,

    // Timer
    timer_start: Instant,
    timer_duration: Duration,

    // Recording state
    is_recording: bool,
    is_processing: bool,
    recording_start: Option<Instant>,
    audio_buffer: Arc<Mutex<Vec<f32>>>,
    audio_stream: Option<cpal::Stream>,
    audio_level: Arc<Mutex<f32>>,
    input_sample_rate: u32,

    // Settings
    show_settings: bool,
    always_on_top: bool,
    timer_duration_mins: u32,
    selected_model: String,
    available_models: Vec<(String, bool)>, // (name, installed)
    ollama_enabled: bool,

    // Audio devices
    audio_devices: Vec<String>,
    selected_device_idx: usize,

    // Error state
    error_message: Option<String>,
    error_time: Option<Instant>,

    // Processing message
    status_message: Option<String>,

    // Background processing channel
    processing_rx: Option<mpsc::Receiver<ProcessingResult>>,

    // Model download state
    download_state: DownloadState,
}

impl Default for FlowStateApp {
    fn default() -> Self {
        let db = database::Database::new().expect("Failed to open database");
        let tasks = database::get_all_tasks(&db).unwrap_or_default();
        let timer_duration_mins = 15;
        let ollama_enabled = database::get_ollama_enabled(&db).unwrap_or(false);

        // Get audio devices
        let host = cpal::default_host();
        let audio_devices: Vec<String> = host
            .input_devices()
            .map(|devices| {
                devices
                    .filter_map(|d| d.name().ok())
                    .collect()
            })
            .unwrap_or_default();

        // Check whisper models (use same path as whisper module)
        let models_dir = dirs::data_dir()
            .unwrap_or_default()
            .join("flowstate")
            .join("whisper_models");
        let available_models: Vec<(String, bool)> = vec![
            ("tiny", 75),
            ("base", 142),
            ("small", 466),
            ("medium", 1500),
        ]
        .into_iter()
        .map(|(name, _size)| {
            let model_path = models_dir.join(format!("ggml-{}.bin", name));
            (name.to_string(), model_path.exists())
        })
        .collect();

        // Auto-select first available model (prefer smaller ones)
        let selected_model = available_models
            .iter()
            .find(|(_, installed)| *installed)
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| "tiny".to_string());

        eprintln!("🔧 Selected Whisper model: {} (available: {:?})", selected_model, available_models);

        Self {
            db,
            tasks,
            timer_start: Instant::now(),
            timer_duration: Duration::from_secs(timer_duration_mins as u64 * 60),
            is_recording: false,
            is_processing: false,
            recording_start: None,
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
            audio_stream: None,
            audio_level: Arc::new(Mutex::new(0.0)),
            input_sample_rate: 48000, // Default, will be updated when recording starts
            show_settings: false,
            always_on_top: false,
            timer_duration_mins,
            selected_model,
            available_models,
            ollama_enabled,
            audio_devices,
            selected_device_idx: 0,
            error_message: None,
            error_time: None,
            status_message: None,
            processing_rx: None,
            download_state: DownloadState::default(),
        }
    }
}

impl FlowStateApp {
    fn reload_tasks(&mut self) {
        self.tasks = database::get_all_tasks(&self.db).unwrap_or_default();
    }

    fn refresh_models(&mut self) {
        let models_dir = dirs::data_dir()
            .unwrap_or_default()
            .join("flowstate")
            .join("whisper_models");
        self.available_models = vec![
            ("tiny", 75),
            ("base", 142),
            ("small", 466),
            ("medium", 1500),
        ]
        .into_iter()
        .map(|(name, _size)| {
            let model_path = models_dir.join(format!("ggml-{}.bin", name));
            (name.to_string(), model_path.exists())
        })
        .collect();
    }

    fn start_download(&mut self, model_name: &str) {
        let state = self.download_state.clone();
        let model = model_name.to_string();

        // Set downloading state
        *state.is_downloading.lock().unwrap() = true;
        *state.current_model.lock().unwrap() = Some(model.clone());
        *state.progress.lock().unwrap() = 0.0;
        *state.downloaded_mb.lock().unwrap() = 0.0;
        *state.error.lock().unwrap() = None;
        *state.completed.lock().unwrap() = false;

        // Get model URL and size
        let (url, total_size) = match model.as_str() {
            "tiny" => ("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin", 75_000_000u64),
            "base" => ("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin", 142_000_000u64),
            "small" => ("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin", 466_000_000u64),
            "medium" => ("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin", 1_500_000_000u64),
            _ => {
                *state.error.lock().unwrap() = Some("Unknown model".to_string());
                *state.is_downloading.lock().unwrap() = false;
                return;
            }
        };

        *state.total_mb.lock().unwrap() = total_size as f32 / 1_000_000.0;

        let url = url.to_string();

        // Download in background thread
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let models_dir = dirs::data_dir()
                    .unwrap_or_default()
                    .join("flowstate")
                    .join("whisper_models");

                if let Err(e) = std::fs::create_dir_all(&models_dir) {
                    *state.error.lock().unwrap() = Some(format!("Failed to create directory: {}", e));
                    *state.is_downloading.lock().unwrap() = false;
                    return;
                }

                let model_path = models_dir.join(format!("ggml-{}.bin", model));

                // Download with reqwest
                let client = reqwest::Client::new();
                let response = match client.get(&url).send().await {
                    Ok(r) => r,
                    Err(e) => {
                        *state.error.lock().unwrap() = Some(format!("Download failed: {}", e));
                        *state.is_downloading.lock().unwrap() = false;
                        return;
                    }
                };

                if !response.status().is_success() {
                    *state.error.lock().unwrap() = Some(format!("HTTP error: {}", response.status()));
                    *state.is_downloading.lock().unwrap() = false;
                    return;
                }

                let total = response.content_length().unwrap_or(total_size);
                *state.total_mb.lock().unwrap() = total as f32 / 1_000_000.0;

                let mut file = match std::fs::File::create(&model_path) {
                    Ok(f) => f,
                    Err(e) => {
                        *state.error.lock().unwrap() = Some(format!("Failed to create file: {}", e));
                        *state.is_downloading.lock().unwrap() = false;
                        return;
                    }
                };

                use futures_util::StreamExt;
                use std::io::Write;

                let mut stream = response.bytes_stream();
                let mut downloaded: u64 = 0;

                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(bytes) => {
                            if let Err(e) = file.write_all(&bytes) {
                                *state.error.lock().unwrap() = Some(format!("Write error: {}", e));
                                *state.is_downloading.lock().unwrap() = false;
                                return;
                            }
                            downloaded += bytes.len() as u64;
                            *state.downloaded_mb.lock().unwrap() = downloaded as f32 / 1_000_000.0;
                            *state.progress.lock().unwrap() = downloaded as f32 / total as f32;
                        }
                        Err(e) => {
                            *state.error.lock().unwrap() = Some(format!("Download error: {}", e));
                            *state.is_downloading.lock().unwrap() = false;
                            return;
                        }
                    }
                }

                *state.completed.lock().unwrap() = true;
                *state.is_downloading.lock().unwrap() = false;
            });
        });
    }

    fn start_recording(&mut self) {
        let host = cpal::default_host();

        eprintln!("🎙️ Starting recording with device index: {}", self.selected_device_idx);

        let device = if self.selected_device_idx == 0 {
            eprintln!("  → Using default input device");
            host.default_input_device()
        } else {
            eprintln!("  → Using device at index {}", self.selected_device_idx - 1);
            host.input_devices()
                .ok()
                .and_then(|mut devices| devices.nth(self.selected_device_idx - 1))
        };

        let Some(device) = device else {
            eprintln!("❌ No audio device found!");
            self.error_message = Some("No audio device found".to_string());
            self.error_time = Some(Instant::now());
            return;
        };

        eprintln!("  → Device: {:?}", device.name());

        let supported_config = match device.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                self.error_message = Some(format!("Failed to get audio config: {}", e));
                self.error_time = Some(Instant::now());
                return;
            }
        };

        let buffer = self.audio_buffer.clone();
        let audio_level = self.audio_level.clone();
        let sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels() as usize;
        let sample_format = supported_config.sample_format();

        // Store sample rate for resampling later
        self.input_sample_rate = sample_rate;
        eprintln!("🎤 Recording at {} Hz, {} channels, format: {:?}", sample_rate, channels, sample_format);

        // Clear buffer and reset level
        buffer.lock().unwrap().clear();
        *audio_level.lock().unwrap() = 0.0;

        // Build stream based on sample format
        let config: cpal::StreamConfig = supported_config.into();

        let stream = match sample_format {
            cpal::SampleFormat::I16 => {
                let buffer = buffer.clone();
                let audio_level = audio_level.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        // Convert i16 to f32 and then to mono
                        let mono: Vec<f32> = data
                            .chunks(channels)
                            .map(|chunk| {
                                let sum: f32 = chunk.iter().map(|&s| s as f32 / 32768.0).sum();
                                sum / channels as f32
                            })
                            .collect();

                        // Calculate RMS level for visualization
                        if !mono.is_empty() {
                            let sum_squares: f32 = mono.iter().map(|s| s * s).sum();
                            let rms = (sum_squares / mono.len() as f32).sqrt();
                            let level = (rms * 4.0).min(1.0);
                            if let Ok(mut lvl) = audio_level.lock() {
                                *lvl = if level > *lvl { level } else { *lvl * 0.9 + level * 0.1 };
                            }
                        }

                        let mut buf = buffer.lock().unwrap();
                        buf.extend(mono);
                    },
                    |err| eprintln!("Audio error: {}", err),
                    None,
                )
            }
            cpal::SampleFormat::F32 => {
                let buffer = buffer.clone();
                let audio_level = audio_level.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        // Convert to mono
                        let mono: Vec<f32> = data
                            .chunks(channels)
                            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
                            .collect();

                        // Calculate RMS level for visualization
                        if !mono.is_empty() {
                            let sum_squares: f32 = mono.iter().map(|s| s * s).sum();
                            let rms = (sum_squares / mono.len() as f32).sqrt();
                            let level = (rms * 4.0).min(1.0);
                            if let Ok(mut lvl) = audio_level.lock() {
                                *lvl = if level > *lvl { level } else { *lvl * 0.9 + level * 0.1 };
                            }
                        }

                        let mut buf = buffer.lock().unwrap();
                        buf.extend(mono);
                    },
                    |err| eprintln!("Audio error: {}", err),
                    None,
                )
            }
            _ => {
                self.error_message = Some(format!("Unsupported sample format: {:?}", sample_format));
                self.error_time = Some(Instant::now());
                return;
            }
        };

        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                self.error_message = Some(format!("Failed to start recording: {}", e));
                self.error_time = Some(Instant::now());
                return;
            }
        };

        if let Err(e) = stream.play() {
            self.error_message = Some(format!("Failed to play stream: {}", e));
            self.error_time = Some(Instant::now());
            return;
        }

        self.audio_stream = Some(stream);
        self.is_recording = true;
        self.recording_start = Some(Instant::now());
    }

    fn stop_recording(&mut self) {
        self.is_recording = false;
        self.audio_stream = None;
        self.recording_start = None;

        // Reset audio level
        *self.audio_level.lock().unwrap() = 0.0;

        // Get audio data
        let audio_data: Vec<f32> = {
            let buf = self.audio_buffer.lock().unwrap();
            buf.clone()
        };

        // Analyze the captured audio
        let duration_secs = audio_data.len() as f32 / self.input_sample_rate as f32;
        let (min_val, max_val, rms) = if audio_data.is_empty() {
            (0.0, 0.0, 0.0)
        } else {
            let min = audio_data.iter().cloned().fold(f32::INFINITY, f32::min);
            let max = audio_data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let sum_sq: f32 = audio_data.iter().map(|s| s * s).sum();
            let rms = (sum_sq / audio_data.len() as f32).sqrt();
            (min, max, rms)
        };

        eprintln!("🛑 Stopped recording:");
        eprintln!("   Samples: {}", audio_data.len());
        eprintln!("   Duration: {:.2}s", duration_secs);
        eprintln!("   Min: {:.4}, Max: {:.4}, RMS: {:.4}", min_val, max_val, rms);

        if audio_data.is_empty() {
            self.error_message = Some("No audio recorded".to_string());
            self.error_time = Some(Instant::now());
            return;
        }

        // Check minimum duration (at least 0.3 seconds)
        if duration_secs < 0.3 {
            self.error_message = Some(format!("Recording too short ({:.1}s). Hold longer.", duration_secs));
            self.error_time = Some(Instant::now());
            return;
        }

        // Check if audio has enough volume (use max amplitude, more reliable than RMS)
        let max_amplitude = max_val.abs().max(min_val.abs());
        if max_amplitude < 0.01 {
            self.error_message = Some(format!("Audio too quiet (peak: {:.4}). Check mic volume.", max_amplitude));
            self.error_time = Some(Instant::now());
            return;
        }

        self.is_processing = true;
        self.status_message = Some("Loading model...".to_string());

        let model = self.selected_model.clone();
        let ollama_enabled = self.ollama_enabled;
        let input_rate = self.input_sample_rate;

        // Create channel for results
        let (tx, rx) = mpsc::channel();
        self.processing_rx = Some(rx);

        // Process in background thread
        thread::spawn(move || {
            // Downsample to 16kHz
            let input_rate = input_rate as f32;
            let output_rate = 16000.0;
            eprintln!("🔄 Resampling from {} Hz to {} Hz ({} samples)", input_rate, output_rate, audio_data.len());

            let resampled = if (input_rate - output_rate).abs() < 1.0 {
                audio_data
            } else {
                let ratio = input_rate / output_rate;
                let new_len = (audio_data.len() as f32 / ratio) as usize;
                let mut resampled = Vec::with_capacity(new_len);
                for i in 0..new_len {
                    let src_idx = i as f32 * ratio;
                    let idx = src_idx as usize;
                    let frac = src_idx - idx as f32;
                    let sample = if idx + 1 < audio_data.len() {
                        audio_data[idx] * (1.0 - frac) + audio_data[idx + 1] * frac
                    } else if idx < audio_data.len() {
                        audio_data[idx]
                    } else {
                        0.0
                    };
                    resampled.push(sample);
                }
                resampled
            };

            eprintln!("📊 Resampled to {} samples", resampled.len());

            // Transcribe
            match whisper::transcribe_audio(&resampled, &model) {
                Ok(transcript) => {
                    eprintln!("📝 Transcript: '{}'", transcript);

                    if transcript.trim().is_empty() {
                        let _ = tx.send(ProcessingResult::Error(
                            "No speech detected. Try speaking louder or closer to the mic.".to_string()
                        ));
                        let _ = tx.send(ProcessingResult::Done);
                        return;
                    }

                    let _ = tx.send(ProcessingResult::Transcript(transcript.clone()));

                    // Parse tasks
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    match rt.block_on(ollama::parse_transcript(&transcript, ollama_enabled)) {
                        Ok(parsed_tasks) => {
                            eprintln!("✅ Parsed {} tasks", parsed_tasks.len());
                            let _ = tx.send(ProcessingResult::Tasks(parsed_tasks));
                        }
                        Err(e) => {
                            let _ = tx.send(ProcessingResult::Error(format!("Parse error: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(ProcessingResult::Error(format!("Transcription error: {}", e)));
                }
            }
            let _ = tx.send(ProcessingResult::Done);
        });
    }

    fn timer_remaining(&self) -> Duration {
        let elapsed = self.timer_start.elapsed();
        if elapsed >= self.timer_duration {
            Duration::ZERO
        } else {
            self.timer_duration - elapsed
        }
    }

    fn reset_timer(&mut self) {
        self.timer_start = Instant::now();
        self.timer_duration = Duration::from_secs(self.timer_duration_mins as u64 * 60);
    }
}

impl eframe::App for FlowStateApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Check timer expiry
        if self.timer_remaining() == Duration::ZERO && self.timer_duration_mins > 0 {
            // Play sound (beep)
            print!("\x07"); // ASCII bell
            self.reset_timer();
        }

        // Clear old errors
        if let Some(error_time) = self.error_time {
            if error_time.elapsed() > Duration::from_secs(5) {
                self.error_message = None;
                self.error_time = None;
            }
        }

        // Check for background processing results
        if let Some(rx) = self.processing_rx.take() {
            let mut results = Vec::new();
            while let Ok(result) = rx.try_recv() {
                results.push(result);
            }

            let mut done = false;
            for result in results {
                match result {
                    ProcessingResult::Transcript(transcript) => {
                        self.status_message = Some(format!("Transcribed: {}", transcript));
                    }
                    ProcessingResult::Tasks(parsed_tasks) => {
                        if parsed_tasks.is_empty() {
                            self.error_message = Some("No tasks found in transcript".to_string());
                            self.error_time = Some(Instant::now());
                        } else {
                            for task in &parsed_tasks {
                                eprintln!("  → Adding task: '{}' (completed: {})", task.text, task.completed);
                                if task.completed {
                                    let _ = database::find_and_complete_task(&self.db, &task.text);
                                } else {
                                    let _ = database::add_task(&self.db, &task.text);
                                }
                            }
                            self.reload_tasks();
                            self.status_message = Some(format!("Added {} task(s)", parsed_tasks.len()));
                        }
                    }
                    ProcessingResult::Error(e) => {
                        self.error_message = Some(e);
                        self.error_time = Some(Instant::now());
                    }
                    ProcessingResult::Done => {
                        done = true;
                    }
                }
            }

            if done {
                self.is_processing = false;
                self.status_message = None;
            } else {
                // Put the receiver back if not done
                self.processing_rx = Some(rx);
            }
        }

        // Set always on top
        // Note: eframe 0.29 doesn't have direct always_on_top, would need platform-specific code

        // Dark theme
        ctx.set_visuals(egui::Visuals::dark());

        // Timer bar at top
        egui::TopBottomPanel::top("timer_bar").show(ctx, |ui| {
            let remaining = self.timer_remaining();
            let total = self.timer_duration.as_secs_f32();
            let progress = if total > 0.0 {
                remaining.as_secs_f32() / total
            } else {
                0.0
            };

            ui.horizontal(|ui| {
                let bar_width = ui.available_width() - 60.0;
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(bar_width, 4.0),
                    egui::Sense::hover(),
                );

                // Background
                ui.painter().rect_filled(rect, 0.0, egui::Color32::from_gray(40));

                // Progress
                let progress_rect = egui::Rect::from_min_size(
                    rect.min,
                    egui::vec2(rect.width() * progress, rect.height()),
                );
                ui.painter().rect_filled(
                    progress_rect,
                    0.0,
                    egui::Color32::from_rgb(74, 158, 255),
                );

                // Time text
                let mins = remaining.as_secs() / 60;
                let secs = remaining.as_secs() % 60;
                ui.label(format!("{}:{:02}", mins, secs));
            });
        });

        // Main content
        egui::CentralPanel::default().show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.heading("FlowState");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⚙").clicked() {
                        self.show_settings = true;
                    }
                    let pin_text = if self.always_on_top { "📌" } else { "📍" };
                    if ui.button(pin_text).clicked() {
                        self.always_on_top = !self.always_on_top;
                    }
                });
            });

            ui.add_space(8.0);

            // Record button
            ui.vertical_centered(|ui| {
                let button_size = egui::vec2(100.0, 100.0);
                let (rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());

                let is_hovered = response.hovered();

                // Get current audio level
                let current_level = *self.audio_level.lock().unwrap();

                // Draw audio level ring when recording
                if self.is_recording && current_level > 0.01 {
                    let level_radius = 42.0 + current_level * 8.0; // 42-50 range
                    let level_alpha = (current_level * 200.0) as u8;
                    ui.painter().circle_stroke(
                        rect.center(),
                        level_radius,
                        egui::Stroke::new(3.0, egui::Color32::from_rgba_unmultiplied(239, 68, 68, level_alpha)),
                    );
                }

                let bg_color = if self.is_recording {
                    egui::Color32::from_rgb(239, 68, 68) // Red when recording
                } else if self.is_processing {
                    egui::Color32::from_gray(60)
                } else if is_hovered {
                    egui::Color32::from_gray(50)
                } else {
                    egui::Color32::from_gray(42)
                };

                ui.painter().circle_filled(rect.center(), 40.0, bg_color);

                // Icon
                if self.is_processing {
                    // Spinner would go here - just show text for now
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "...",
                        egui::FontId::proportional(24.0),
                        egui::Color32::WHITE,
                    );
                } else if self.is_recording {
                    // Stop icon (square)
                    let square_size = 20.0;
                    let square_rect = egui::Rect::from_center_size(
                        rect.center(),
                        egui::vec2(square_size, square_size),
                    );
                    ui.painter().rect_filled(square_rect, 2.0, egui::Color32::WHITE);
                } else {
                    // Record icon (circle)
                    ui.painter().circle_filled(rect.center(), 20.0, egui::Color32::WHITE);
                }

                // Handle hold-to-record: start on press, stop on release
                // Use global mouse state so releasing anywhere stops recording
                let mouse_down = ctx.input(|i| i.pointer.primary_down());

                if response.is_pointer_button_down_on() && !self.is_recording && !self.is_processing {
                    // Mouse pressed on button - start recording
                    self.start_recording();
                }

                if self.is_recording && !mouse_down {
                    // Mouse released anywhere - stop recording
                    self.stop_recording();
                }

                // Recording time / hint
                if self.is_recording {
                    if let Some(start) = self.recording_start {
                        let secs = start.elapsed().as_secs();
                        ui.label(format!("● Recording {}:{:02}", secs / 60, secs % 60));
                    }
                } else if self.is_processing {
                    ui.label("Processing...");
                } else {
                    ui.label(egui::RichText::new("Hold to record").color(egui::Color32::GRAY));
                }
            });

            ui.add_space(8.0);

            // Error message
            if let Some(ref error) = self.error_message {
                ui.colored_label(egui::Color32::from_rgb(248, 113, 113), error);
            }

            // Status message
            if let Some(ref status) = self.status_message {
                ui.label(status);
            }

            ui.add_space(8.0);

            // Task list
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut tasks_to_toggle = Vec::new();
                let mut tasks_to_delete = Vec::new();

                for task in &self.tasks {
                    ui.horizontal(|ui| {
                        let mut completed = task.completed;
                        if ui.checkbox(&mut completed, "").changed() {
                            tasks_to_toggle.push(task.id);
                        }

                        let text = if task.completed {
                            egui::RichText::new(&task.text)
                                .strikethrough()
                                .color(egui::Color32::GRAY)
                        } else {
                            egui::RichText::new(&task.text)
                        };
                        ui.label(text);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("×").clicked() {
                                tasks_to_delete.push(task.id);
                            }
                        });
                    });
                }

                // Apply changes
                let should_reload = !tasks_to_toggle.is_empty() || !tasks_to_delete.is_empty();
                for id in tasks_to_toggle {
                    let _ = database::toggle_task(&self.db, id);
                }
                for id in tasks_to_delete {
                    let _ = database::delete_task(&self.db, id);
                }
                if should_reload {
                    self.reload_tasks();
                }
            });
        });

        // Settings window
        if self.show_settings {
            egui::Window::new("Settings")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    // Timer duration
                    ui.horizontal(|ui| {
                        ui.label("Timer (minutes):");
                        if ui.add(egui::Slider::new(&mut self.timer_duration_mins, 0..=60)).changed() {
                            self.reset_timer();
                        }
                    });

                    ui.add_space(8.0);

                    // Audio device
                    ui.label("Microphone:");
                    let selected_device_name = if self.selected_device_idx == 0 {
                        "Default".to_string()
                    } else {
                        self.audio_devices
                            .get(self.selected_device_idx - 1)
                            .cloned()
                            .unwrap_or_else(|| "Unknown".to_string())
                    };
                    egui::ComboBox::from_id_salt("audio_device")
                        .selected_text(&selected_device_name)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.selected_device_idx, 0, "Default");
                            for (idx, name) in self.audio_devices.iter().enumerate() {
                                ui.selectable_value(&mut self.selected_device_idx, idx + 1, name);
                            }
                        });

                    ui.add_space(8.0);

                    // Whisper model
                    ui.label("Whisper Model:");

                    // Check if download completed and refresh
                    if *self.download_state.completed.lock().unwrap() {
                        self.refresh_models();
                        *self.download_state.completed.lock().unwrap() = false;
                    }

                    let is_downloading = *self.download_state.is_downloading.lock().unwrap();
                    let current_downloading = self.download_state.current_model.lock().unwrap().clone();

                    // Clone available_models to avoid borrow issues
                    let models_snapshot: Vec<_> = self.available_models.clone();
                    let mut model_to_download: Option<String> = None;

                    for (name, installed) in &models_snapshot {
                        ui.horizontal(|ui| {
                            let selected = self.selected_model == *name;
                            if ui.radio(selected && *installed, name).clicked() && *installed {
                                self.selected_model = name.clone();
                            }

                            let is_this_downloading = current_downloading.as_ref() == Some(name);

                            if *installed {
                                ui.colored_label(egui::Color32::from_rgb(74, 222, 128), "✓ Installed");
                            } else if is_this_downloading {
                                // Show progress
                                let progress = *self.download_state.progress.lock().unwrap();
                                let downloaded = *self.download_state.downloaded_mb.lock().unwrap();
                                let total = *self.download_state.total_mb.lock().unwrap();
                                ui.add(egui::ProgressBar::new(progress).text(format!("{:.0}/{:.0} MB", downloaded, total)));
                            } else if !is_downloading {
                                // Show download button
                                if ui.small_button("Download").clicked() {
                                    model_to_download = Some(name.clone());
                                }
                            } else {
                                ui.colored_label(egui::Color32::GRAY, "—");
                            }
                        });
                    }

                    // Start download if requested (after the loop to avoid borrow issues)
                    if let Some(model) = model_to_download {
                        self.start_download(&model);
                    }

                    // Show download error if any
                    if let Some(ref error) = *self.download_state.error.lock().unwrap() {
                        ui.colored_label(egui::Color32::from_rgb(248, 113, 113), error);
                    }

                    ui.add_space(8.0);

                    // Ollama toggle
                    ui.checkbox(&mut self.ollama_enabled, "Use Ollama for better parsing");
                    if self.ollama_enabled {
                        ui.label(egui::RichText::new("Slower but more accurate").small().color(egui::Color32::GRAY));
                    }

                    ui.add_space(16.0);

                    if ui.button("Close").clicked() {
                        // Save settings
                        let _ = database::set_ollama_enabled(&self.db, self.ollama_enabled);
                        self.show_settings = false;
                    }
                });
        }

        // Only repaint when needed (not continuously!)
        // This is the key to 0% CPU - we only repaint on events
        let is_downloading = *self.download_state.is_downloading.lock().unwrap();

        if self.is_recording || is_downloading || self.is_processing {
            // Repaint every 100ms while recording, downloading, or processing
            ctx.request_repaint_after(Duration::from_millis(100));
        } else {
            // When idle, only repaint every 10 seconds for timer
            ctx.request_repaint_after(Duration::from_secs(10));
        }
    }
}

/// Generate a 3D-style red record button icon with "FS" (32x32 RGBA)
fn create_record_icon() -> egui::IconData {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    // Button positioned slightly offset (center-right, center-bottom)
    let cx = 17.0f32;
    let cy = 17.0f32;
    let radius = 13.0f32;

    // Light source from top-left
    let light_x = -0.5f32;
    let light_y = -0.7f32;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            let idx = ((y * size + x) * 4) as usize;

            if dist <= radius {
                // Normalize direction from center
                let nx = if dist > 0.1 { dx / dist } else { 0.0 };
                let ny = if dist > 0.1 { dy / dist } else { 0.0 };

                // 3D shading: dot product with light direction
                let dot = -(nx * light_x + ny * light_y);
                let shade = (dot * 0.4 + 0.6).clamp(0.3, 1.0);

                // Edge darkening for depth
                let edge_factor = (1.0 - (dist / radius).powf(2.0)).clamp(0.0, 1.0);
                let final_shade = shade * (0.7 + 0.3 * edge_factor);

                // Base red color with shading
                let r = (220.0 * final_shade) as u8;
                let g = (50.0 * final_shade) as u8;
                let b = (50.0 * final_shade) as u8;

                // Anti-aliasing at edge
                let alpha = if dist > radius - 1.0 {
                    ((radius - dist + 1.0) * 255.0) as u8
                } else {
                    255
                };

                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = alpha;
            }

            // Add highlight spot (top-left of button)
            let hx = cx - 5.0;
            let hy = cy - 5.0;
            let hdist = ((x as f32 - hx).powi(2) + (y as f32 - hy).powi(2)).sqrt();
            if hdist < 4.0 && dist < radius - 2.0 {
                let intensity = (1.0 - hdist / 4.0).powi(2);
                let blend = (intensity * 0.6).min(1.0);
                rgba[idx] = (rgba[idx] as f32 + (255.0 - rgba[idx] as f32) * blend) as u8;
                rgba[idx + 1] = (rgba[idx + 1] as f32 + (200.0 - rgba[idx + 1] as f32) * blend) as u8;
                rgba[idx + 2] = (rgba[idx + 2] as f32 + (200.0 - rgba[idx + 2] as f32) * blend) as u8;
            }
        }
    }

    // Draw "FS" letters in top-left corner
    // Simple bitmap font for "F" and "S"
    let letter_color = [255u8, 255, 255, 255]; // White

    // "F" at position (2, 3) - 5x7 pixels
    let f_pattern: [(i32, i32); 12] = [
        (0,0), (1,0), (2,0), (3,0),  // top bar
        (0,1), (0,2),                 // vertical
        (0,3), (1,3), (2,3),          // middle bar
        (0,4), (0,5), (0,6),          // vertical continued
    ];
    for (fx, fy) in f_pattern {
        let px = (2 + fx) as u32;
        let py = (2 + fy) as u32;
        if px < size && py < size {
            let idx = ((py * size + px) * 4) as usize;
            rgba[idx] = letter_color[0];
            rgba[idx + 1] = letter_color[1];
            rgba[idx + 2] = letter_color[2];
            rgba[idx + 3] = letter_color[3];
        }
    }

    // "S" at position (7, 3) - 4x7 pixels
    let s_pattern: [(i32, i32); 13] = [
        (1,0), (2,0), (3,0),          // top bar
        (0,1),                         // left top
        (0,2), (1,2), (2,2),          // middle bar start
        (3,3), (3,4),                  // right bottom
        (0,5), (1,5), (2,5), (3,5),   // bottom bar (added one more)
    ];
    for (sx, sy) in s_pattern {
        let px = (7 + sx) as u32;
        let py = (2 + sy) as u32;
        if px < size && py < size {
            let idx = ((py * size + px) * 4) as usize;
            rgba[idx] = letter_color[0];
            rgba[idx + 1] = letter_color[1];
            rgba[idx + 2] = letter_color[2];
            rgba[idx + 3] = letter_color[3];
        }
    }

    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}

fn main() -> eframe::Result<()> {
    let icon = create_record_icon();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([320.0, 480.0])
            .with_min_inner_size([280.0, 400.0])
            .with_title("FlowState")
            .with_icon(std::sync::Arc::new(icon)),
        ..Default::default()
    };

    eframe::run_native(
        "FlowState",
        options,
        Box::new(|_cc| Ok(Box::new(FlowStateApp::default()))),
    )
}
