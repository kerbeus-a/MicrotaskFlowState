// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! FlowState: voice-first micro-task manager. Run with `cargo run --release`.

mod app;
mod audio;
mod database;
mod icon;
mod llm;
mod models;
mod parser;
mod qwen2;
mod theme;
mod whisper;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        std::process::exit(cli(&args));
    }

    let icon_size = 64;
    let icon = egui::IconData {
        rgba: icon::render_rgba(icon_size),
        width: icon_size,
        height: icon_size,
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([380.0, 660.0])
            .with_min_inner_size([340.0, 520.0])
            .with_title("FlowState")
            .with_app_id("flowstate")
            .with_icon(std::sync::Arc::new(icon)),
        persist_window: true,
        ..Default::default()
    };

    eframe::run_native(
        "FlowState",
        options,
        Box::new(|cc| {
            #[cfg(windows)]
            set_taskbar_icon(cc);
            Ok(Box::new(app::FlowStateApp::new(&cc.egui_ctx)))
        }),
    )
}

/// Diagnostics without the UI:
///   flowstate --download <model-id>
///   flowstate --transcribe <file.wav> [rules|builtin]
fn cli(args: &[String]) -> i32 {
    match args {
        [flag, id] if flag == "--download" => {
            let Some(spec) = models::find(id) else {
                let ids: Vec<_> = models::CATALOG.iter().map(|m| m.id).collect();
                eprintln!("Unknown model. Available: {}", ids.join(", "));
                return 2;
            };
            let progress = models::start_download(spec, || {});
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let p = progress.lock().unwrap();
                eprint!("\r{} / {}   ", models::format_size(p.downloaded), models::format_size(p.total));
                if p.finished {
                    eprintln!();
                    return match &p.error {
                        Some(e) => {
                            eprintln!("Failed: {e}");
                            1
                        }
                        None => {
                            println!("Installed {} in {}", spec.name, models::models_dir().display());
                            0
                        }
                    };
                }
            }
        }
        [flag, wav, rest @ ..] if flag == "--transcribe" => {
            let engine = parser::ParseEngine::from_str(rest.first().map(String::as_str).unwrap_or("rules"));
            match transcribe_file(wav, engine) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }
        }
        _ => {
            eprintln!("Usage: flowstate [--download <model-id> | --transcribe <file.wav> [rules|builtin]]");
            2
        }
    }
}

fn transcribe_file(path: &str, engine: parser::ParseEngine) -> Result<(), String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|s| s.unwrap_or(0) as f32 / scale).collect()
        }
    };
    let mono = raw.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32).collect();
    let captured = audio::Captured { samples: mono, sample_rate: spec.sample_rate };

    let speech = models::find(models::DEFAULT_SPEECH_MODEL)
        .filter(|m| m.is_installed())
        .or_else(|| models::of_kind(models::ModelKind::Speech).find(|m| m.is_installed()))
        .ok_or("No speech model installed (try --download whisper-base.en)")?;

    let t = std::time::Instant::now();
    let text = whisper::transcribe_local(&speech.path(0), &captured.to_whisper_rate())?;
    println!("Transcript ({:.1}s, {}): {text}", t.elapsed().as_secs_f32(), speech.name);

    let language_model = models::of_kind(models::ModelKind::Language)
        .find(|m| m.is_installed())
        .map(|m| m.id.to_string())
        .unwrap_or_default();
    if engine == parser::ParseEngine::BuiltIn {
        // The app does this in the background at startup.
        let t = std::time::Instant::now();
        parser::warm_up_builtin(&language_model)?;
        println!("Warm-up: {:.1}s", t.elapsed().as_secs_f32());
    }
    let t = std::time::Instant::now();
    let outcome = parser::parse_transcript(
        &text,
        &parser::ParseOptions {
            engine,
            language_model,
            ollama_url: String::new(),
            ollama_model: String::new(),
        },
    );
    println!("Tasks ({:.1}s, {:?}):", t.elapsed().as_secs_f32(), engine);
    for task in outcome.tasks {
        println!("  [{}] {}", if task.completed { "x" } else { " " }, task.text);
    }
    if let Some(reason) = outcome.fallback_reason {
        println!("  (fell back to rules: {reason})");
    }
    Ok(())
}

/// winit only sets the small (title bar) icon on Windows, which leaves the taskbar
/// showing a generic window. Load the large icon embedded by build.rs and apply it.
#[cfg(windows)]
fn set_taskbar_icon(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON,
        LR_DEFAULTCOLOR, SM_CXICON, SM_CXSMICON, WM_SETICON,
    };

    let Ok(handle) = cc.window_handle() else { return };
    let RawWindowHandle::Win32(win) = handle.as_raw() else { return };
    let hwnd = win.hwnd.get() as windows_sys::Win32::Foundation::HWND;

    // SAFETY: plain Win32 calls on our own window; resource id 1 is the icon from build.rs.
    unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        for (kind, metric) in [(ICON_BIG, SM_CXICON), (ICON_SMALL, SM_CXSMICON)] {
            let size = GetSystemMetrics(metric);
            let hicon = LoadImageW(module, 1 as _, IMAGE_ICON, size, size, LR_DEFAULTCOLOR);
            if !hicon.is_null() {
                SendMessageW(hwnd, WM_SETICON, kind as usize, hicon as isize);
            }
        }
    }
}
