//! FlowState UI and app state.

use crate::audio::{self, Captured, Recording};
use crate::database::{self, Database, Task};
use crate::models::{self, ModelKind, ModelSpec, SharedProgress};
use crate::parser::{self, ParseEngine, ParseOptions, ParseOutcome};
use crate::theme::{self, *};
use crate::whisper;
use eframe::egui::{self, Align, Align2, Color32, CursorIcon, Layout, Rect, RichText, Sense, Stroke, Vec2};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const MAX_TASK_LEN: usize = 500;
/// Presses longer than this are push-to-talk; shorter ones toggle recording.
const HOLD_THRESHOLD: Duration = Duration::from_millis(350);

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Tasks,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Active,
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SpeechEngine {
    OnDevice,
    Remote,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToastKind {
    Info,
    Success,
    Error,
}

struct Toast {
    text: String,
    kind: ToastKind,
    born: Instant,
}

enum JobMsg {
    Stage(&'static str),
    Transcript(String),
    Parsed(ParseOutcome),
    Failed(String),
}

struct Job {
    rx: mpsc::Receiver<JobMsg>,
    stage: &'static str,
}

struct Press {
    at: Instant,
    started_recording: bool,
}

pub struct FlowStateApp {
    db: Database,
    tasks: Vec<Task>,
    view: View,
    tab: Tab,

    // Check-in timer
    timer_mins: u32,
    timer_started: Instant,
    timer_paused: Option<Duration>,
    chime: bool,

    // Voice capture
    recording: Option<Recording>,
    press: Option<Press>,
    job: Option<Job>,
    last_transcript: Option<(String, Instant)>,

    // Settings
    always_on_top: bool,
    applied_on_top: Option<bool>,
    mic_devices: Vec<String>,
    mic_device: Option<String>,
    speech_engine: SpeechEngine,
    speech_model: String,
    whisper_url: String,
    parse_engine: ParseEngine,
    language_model: String,
    ollama_url: String,
    ollama_model: String,
    downloads: Vec<SharedProgress>,

    // Editing
    new_task_text: String,
    focus_composer: bool,
    editing: Option<(i64, String)>,
    focus_editor: bool,

    toasts: Vec<Toast>,
}

impl FlowStateApp {
    pub fn new(ctx: &egui::Context) -> Self {
        theme::apply(ctx);

        let db = Database::new().expect("Failed to open database");
        let tasks = database::get_all_tasks(&db).unwrap_or_default();
        let get = |k: &str| database::get_setting(&db, k);

        let env_whisper = std::env::var("WHISPER_URL").ok();
        let speech_engine = match get("speech_engine").as_deref() {
            Some("remote") => SpeechEngine::Remote,
            Some(_) => SpeechEngine::OnDevice,
            None if env_whisper.is_some() => SpeechEngine::Remote,
            None => SpeechEngine::OnDevice,
        };
        // Migrate the old on/off Ollama toggle.
        let parse_engine = match get("parser") {
            Some(p) => ParseEngine::from_str(&p),
            None if get("ollama_enabled").as_deref() == Some("true") => ParseEngine::Ollama,
            None => ParseEngine::Rules,
        };

        let app = Self {
            tasks,
            view: View::Tasks,
            tab: Tab::Active,
            timer_mins: get("timer_mins").and_then(|v| v.parse().ok()).unwrap_or(15),
            timer_started: Instant::now(),
            timer_paused: None,
            chime: get("chime").map(|v| v == "true").unwrap_or(true),
            recording: None,
            press: None,
            job: None,
            last_transcript: None,
            always_on_top: get("always_on_top").map(|v| v == "true").unwrap_or(false),
            applied_on_top: None,
            mic_devices: audio::input_device_names(),
            mic_device: get("mic_device").filter(|v| !v.is_empty()),
            speech_engine,
            speech_model: get("speech_model").unwrap_or_else(|| models::DEFAULT_SPEECH_MODEL.into()),
            whisper_url: get("whisper_url").or(env_whisper).unwrap_or_default(),
            parse_engine,
            language_model: get("language_model")
                .unwrap_or_else(|| models::DEFAULT_LANGUAGE_MODEL.into()),
            ollama_url: get("ollama_url")
                .or_else(|| std::env::var("OLLAMA_URL").ok())
                .unwrap_or_else(|| "http://localhost:11434".into()),
            ollama_model: get("ollama_model")
                .or_else(|| std::env::var("OLLAMA_MODEL").ok())
                .unwrap_or_else(|| "llama3.2".into()),
            downloads: Vec::new(),
            new_task_text: String::new(),
            focus_composer: false,
            editing: None,
            focus_editor: false,
            toasts: Vec::new(),
            db,
        };
        app.warm_up_language_model();
        app
    }

    /// Load the on-device language model in the background so the first memo is quick.
    fn warm_up_language_model(&self) {
        if self.parse_engine != ParseEngine::BuiltIn {
            return;
        }
        let id = self.language_model.clone();
        std::thread::spawn(move || {
            if let Err(e) = parser::warm_up_builtin(&id) {
                eprintln!("⚠️ Language model warm-up skipped: {e}");
            }
        });
    }

    // ---------------------------------------------------------------- state helpers

    fn save(&self, key: &str, value: &str) {
        if let Err(e) = database::set_setting(&self.db, key, value) {
            eprintln!("⚠️ Failed to save setting {key}: {e}");
        }
    }

    fn reload_tasks(&mut self) {
        self.tasks = database::get_all_tasks(&self.db).unwrap_or_default();
    }

    fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.toasts.push(Toast {
            text: text.into(),
            kind,
            born: Instant::now(),
        });
        if self.toasts.len() > 3 {
            self.toasts.remove(0);
        }
    }

    fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn timer_total(&self) -> Duration {
        Duration::from_secs(self.timer_mins as u64 * 60)
    }

    fn timer_remaining(&self) -> Duration {
        if let Some(left) = self.timer_paused {
            return left;
        }
        self.timer_total().saturating_sub(self.timer_started.elapsed())
    }

    fn reset_timer(&mut self) {
        self.timer_started = Instant::now();
        if self.timer_paused.is_some() {
            self.timer_paused = Some(self.timer_total());
        }
    }

    fn toggle_timer_pause(&mut self) {
        match self.timer_paused.take() {
            Some(left) => {
                // Resume: shift the start so `left` remains.
                self.timer_started = Instant::now() - (self.timer_total() - left);
            }
            None => self.timer_paused = Some(self.timer_remaining()),
        }
    }

    fn selected_speech_model(&self) -> Option<&'static ModelSpec> {
        models::find(&self.speech_model).filter(|m| m.kind == ModelKind::Speech)
    }

    fn downloading(&self, id: &str) -> Option<SharedProgress> {
        self.downloads
            .iter()
            .find(|p| p.lock().unwrap().model_id == id)
            .cloned()
    }

    fn start_download(&mut self, spec: &'static ModelSpec, ctx: &egui::Context) {
        if self.downloading(spec.id).is_some() {
            return;
        }
        let ctx = ctx.clone();
        self.downloads
            .push(models::start_download(spec, move || ctx.request_repaint()));
    }

    fn voice_ready(&self) -> Result<(), String> {
        match self.speech_engine {
            SpeechEngine::Remote if self.whisper_url.trim().is_empty() => {
                Err("Add a Whisper server URL in Settings".into())
            }
            SpeechEngine::Remote => Ok(()),
            SpeechEngine::OnDevice => match self.selected_speech_model() {
                Some(m) if m.is_installed() => Ok(()),
                _ => Err("Download a speech model to start recording".into()),
            },
        }
    }

    // ---------------------------------------------------------------- voice pipeline

    fn start_recording(&mut self) -> bool {
        if let Err(e) = self.voice_ready() {
            self.toast(ToastKind::Info, e);
            return false;
        }
        match Recording::start(self.mic_device.as_deref()) {
            Ok(rec) => {
                self.recording = Some(rec);
                true
            }
            Err(e) => {
                self.toast(ToastKind::Error, e);
                false
            }
        }
    }

    fn cancel_recording(&mut self) {
        if self.recording.take().is_some() {
            self.toast(ToastKind::Info, "Recording discarded");
        }
    }

    fn stop_recording(&mut self, ctx: &egui::Context) {
        let Some(rec) = self.recording.take() else { return };
        let captured = rec.finish();

        if captured.duration_secs() < 0.4 {
            self.toast(ToastKind::Info, "That was too short — try again");
            return;
        }
        if captured.peak() < 0.01 {
            self.toast(ToastKind::Error, "Didn't hear anything. Check your microphone.");
            return;
        }
        self.process(captured, ctx);
    }

    fn process(&mut self, captured: Captured, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        self.job = Some(Job { rx, stage: "Transcribing…" });

        let speech = match self.speech_engine {
            SpeechEngine::OnDevice => Ok(self.selected_speech_model().map(|m| m.path(0))),
            SpeechEngine::Remote => Err(self.whisper_url.clone()),
        };
        let opts = ParseOptions {
            engine: self.parse_engine,
            language_model: self.language_model.clone(),
            ollama_url: self.ollama_url.clone(),
            ollama_model: self.ollama_model.clone(),
        };
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let send = |msg| {
                let _ = tx.send(msg);
                ctx.request_repaint();
            };
            let samples = captured.to_whisper_rate();
            let transcript = match speech {
                Ok(Some(path)) => whisper::transcribe_local(&path, &samples),
                Ok(None) => Err("No speech model selected".to_string()),
                Err(url) => whisper::transcribe_remote(&url, &samples),
            };
            let transcript = match transcript {
                Ok(t) if t.trim().is_empty() || t.trim().starts_with('[') => {
                    return send(JobMsg::Failed("No speech detected — try again".into()));
                }
                Ok(t) => t,
                Err(e) => return send(JobMsg::Failed(e)),
            };
            send(JobMsg::Transcript(transcript.clone()));
            if opts.engine != ParseEngine::Rules {
                send(JobMsg::Stage("Understanding…"));
            }
            send(JobMsg::Parsed(parser::parse_transcript(&transcript, &opts)));
        });
    }

    fn apply_parsed(&mut self, outcome: ParseOutcome) {
        if let Some(reason) = outcome.fallback_reason {
            self.toast(ToastKind::Error, format!("AI parsing unavailable, used quick rules. {reason}"));
        }
        if outcome.tasks.is_empty() {
            self.toast(ToastKind::Info, "No tasks found in what you said");
            return;
        }
        let (mut added, mut done) = (0, 0);
        for task in &outcome.tasks {
            let result = if task.completed {
                done += 1;
                database::find_and_complete_task(&self.db, &task.text).map(|_| ())
            } else {
                added += 1;
                database::add_task(&self.db, &task.text).map(|_| ())
            };
            if let Err(e) = result {
                self.toast(ToastKind::Error, format!("Couldn't save task: {e}"));
            }
        }
        self.reload_tasks();
        let msg = match (added, done) {
            (a, 0) => format!("Added {a} task{}", plural(a)),
            (0, d) => format!("Completed {d} task{}", plural(d)),
            (a, d) => format!("Added {a}, completed {d}"),
        };
        self.toast(ToastKind::Success, msg);
    }

    fn poll_background(&mut self) {
        if let Some(job) = &mut self.job {
            let mut finished = None;
            let mut transcript = None;
            loop {
                match job.rx.try_recv() {
                    Ok(JobMsg::Stage(s)) => job.stage = s,
                    Ok(JobMsg::Transcript(t)) => transcript = Some(t),
                    Ok(JobMsg::Parsed(o)) => finished = Some(Ok(o)),
                    Ok(JobMsg::Failed(e)) => finished = Some(Err(e)),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        finished.get_or_insert(Err("Processing stopped unexpectedly".into()));
                        break;
                    }
                }
            }
            if let Some(t) = transcript {
                self.last_transcript = Some((t, Instant::now()));
            }
            if let Some(result) = finished {
                self.job = None;
                match result {
                    Ok(outcome) => self.apply_parsed(outcome),
                    Err(e) => self.toast(ToastKind::Error, e),
                }
            }
        }

        let mut finished = Vec::new();
        self.downloads.retain(|p| {
            let p = p.lock().unwrap();
            if p.finished {
                finished.push((p.model_id.clone(), p.error.clone()));
            }
            !p.finished
        });
        for (id, error) in finished {
            let Some(spec) = models::find(&id) else { continue };
            match error {
                Some(e) => self.toast(ToastKind::Error, format!("{}: {e}", spec.name)),
                None => {
                    self.toast(ToastKind::Success, format!("{} is ready", spec.name));
                    // Switch to the freshly downloaded model.
                    match spec.kind {
                        ModelKind::Speech => {
                            self.speech_model = spec.id.into();
                            self.save("speech_model", spec.id);
                        }
                        ModelKind::Language => {
                            self.language_model = spec.id.into();
                            self.save("language_model", spec.id);
                            if self.parse_engine == ParseEngine::Rules {
                                self.parse_engine = ParseEngine::BuiltIn;
                                self.save("parser", ParseEngine::BuiltIn.as_str());
                            }
                            self.warm_up_language_model();
                        }
                    }
                }
            }
        }
    }

    fn tick_timer(&mut self) {
        if self.timer_mins == 0 || self.timer_paused.is_some() {
            return;
        }
        if self.timer_remaining().is_zero() {
            if self.chime {
                audio::play_chime();
            }
            self.toast(ToastKind::Info, "Check-in: what are you working on now?");
            self.reset_timer();
        }
    }

    // ---------------------------------------------------------------- task actions

    fn add_typed_task(&mut self) {
        let text = self.new_task_text.trim().to_string();
        if text.is_empty() {
            return;
        }
        match database::add_task(&self.db, &text) {
            Ok(_) => {
                self.new_task_text.clear();
                self.tab = Tab::Active;
                self.reload_tasks();
            }
            Err(e) => self.toast(ToastKind::Error, format!("Couldn't add task: {e}")),
        }
        self.focus_composer = true;
    }

    fn commit_edit(&mut self) {
        let Some((id, text)) = self.editing.take() else { return };
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        if let Err(e) = database::update_task_text(&self.db, id, text) {
            self.toast(ToastKind::Error, format!("Couldn't save: {e}"));
        }
        self.reload_tasks();
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn fmt_clock(d: Duration) -> String {
    let s = d.as_secs();
    format!("{}:{:02}", s / 60, s % 60)
}

/// "just now", "5m ago", "3h ago", "2d ago" for an SQLite UTC timestamp.
fn relative_time(ts: &str) -> String {
    let Ok(t) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S") else {
        return String::new();
    };
    let secs = (chrono::Utc::now().naive_utc() - t).num_seconds().max(0);
    match secs {
        0..=59 => "just now".into(),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}

// -------------------------------------------------------------------- painting helpers

fn icon_button(
    ui: &mut egui::Ui,
    size: f32,
    tooltip: &str,
    active: bool,
    paint: impl FnOnce(&egui::Painter, Rect, Color32),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let hovered = response.hovered();
    if hovered || active {
        let fill = if active { with_alpha(ACCENT, 0.22) } else { SURFACE_HOVER };
        ui.painter().rect_filled(rect, 8.0, fill);
    }
    let color = if active {
        ACCENT
    } else if hovered {
        TEXT
    } else {
        MUTED
    };
    paint(ui.painter(), rect.shrink(size * 0.27), color);
    response.on_hover_text(tooltip).on_hover_cursor(CursorIcon::PointingHand)
}

fn paint_gear(p: &egui::Painter, r: Rect, c: Color32) {
    let center = r.center();
    let radius = r.width() * 0.5;
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        let dir = Vec2::angled(a);
        p.line_segment(
            [center + dir * radius * 0.55, center + dir * radius],
            Stroke::new(r.width() * 0.16, c),
        );
    }
    p.circle_stroke(center, radius * 0.62, Stroke::new(r.width() * 0.14, c));
    p.circle_filled(center, radius * 0.2, c);
}

fn paint_pin(p: &egui::Painter, r: Rect, c: Color32) {
    let s = Stroke::new(1.8, c);
    let top = r.center_top() + egui::vec2(0.0, r.height() * 0.05);
    let head = Rect::from_center_size(top + egui::vec2(0.0, r.height() * 0.22), egui::vec2(r.width() * 0.5, r.height() * 0.42));
    p.rect_stroke(head, 2.0, s);
    let bar_y = head.bottom();
    p.line_segment([egui::pos2(r.left(), bar_y), egui::pos2(r.right(), bar_y)], s);
    p.line_segment([egui::pos2(r.center().x, bar_y), r.center_bottom()], s);
}

fn paint_back(p: &egui::Painter, r: Rect, c: Color32) {
    let s = Stroke::new(2.0, c);
    let m = r.left_center() + egui::vec2(r.width() * 0.2, 0.0);
    p.line_segment([m, r.center_top() + egui::vec2(r.width() * 0.1, 0.0)], s);
    p.line_segment([m, r.center_bottom() + egui::vec2(r.width() * 0.1, 0.0)], s);
}

fn paint_trash(p: &egui::Painter, r: Rect, c: Color32) {
    let s = Stroke::new(1.6, c);
    let lid_y = r.top() + r.height() * 0.18;
    p.line_segment([egui::pos2(r.left(), lid_y), egui::pos2(r.right(), lid_y)], s);
    let handle = Rect::from_min_max(
        egui::pos2(r.center().x - r.width() * 0.18, r.top()),
        egui::pos2(r.center().x + r.width() * 0.18, lid_y),
    );
    p.rect_stroke(handle, 1.0, s);
    let body = Rect::from_min_max(
        egui::pos2(r.left() + r.width() * 0.12, lid_y),
        egui::pos2(r.right() - r.width() * 0.12, r.bottom()),
    );
    p.rect_stroke(body, 2.0, s);
}

fn paint_pause(p: &egui::Painter, r: Rect, c: Color32) {
    let w = r.width() * 0.28;
    p.rect_filled(Rect::from_min_size(r.left_top() + egui::vec2(r.width() * 0.1, 0.0), egui::vec2(w, r.height())), 1.5, c);
    p.rect_filled(Rect::from_min_size(r.right_top() - egui::vec2(r.width() * 0.1 + w, 0.0), egui::vec2(w, r.height())), 1.5, c);
}

fn paint_play(p: &egui::Painter, r: Rect, c: Color32) {
    p.add(egui::Shape::convex_polygon(
        vec![
            r.left_top() + egui::vec2(r.width() * 0.15, 0.0),
            r.right_center(),
            r.left_bottom() + egui::vec2(r.width() * 0.15, 0.0),
        ],
        c,
        Stroke::NONE,
    ));
}

fn paint_reset(p: &egui::Painter, r: Rect, c: Color32) {
    let center = r.center();
    let radius = r.width() * 0.48;
    let pts: Vec<egui::Pos2> = (0..=24)
        .map(|i| {
            let a = -0.3 + i as f32 / 24.0 * 5.0;
            center + Vec2::angled(a) * radius
        })
        .collect();
    let end = pts[0];
    p.add(egui::Shape::line(pts, Stroke::new(1.8, c)));
    p.add(egui::Shape::convex_polygon(
        vec![end + egui::vec2(-4.0, -4.5), end + egui::vec2(3.5, -3.0), end + egui::vec2(0.5, 3.0)],
        c,
        Stroke::NONE,
    ));
}

fn paint_plus(p: &egui::Painter, r: Rect, c: Color32) {
    let s = Stroke::new(2.2, c);
    p.line_segment([r.left_center(), r.right_center()], s);
    p.line_segment([r.center_top(), r.center_bottom()], s);
}

fn paint_check(p: &egui::Painter, r: Rect, c: Color32, width: f32) {
    p.add(egui::Shape::line(
        vec![
            r.left_center() + egui::vec2(0.0, r.height() * 0.05),
            r.center_bottom() - egui::vec2(r.width() * 0.12, r.height() * 0.1),
            r.right_top() + egui::vec2(0.0, r.height() * 0.15),
        ],
        Stroke::new(width, c),
    ));
}

fn paint_mic(p: &egui::Painter, center: egui::Pos2, scale: f32, c: Color32) {
    let body = Rect::from_center_size(center - egui::vec2(0.0, 5.0 * scale), egui::vec2(12.0 * scale, 20.0 * scale));
    p.rect_filled(body, 6.0 * scale, c);
    let arc_center = center - egui::vec2(0.0, 3.0 * scale);
    let pts: Vec<egui::Pos2> = (0..=20)
        .map(|i| {
            let a = i as f32 / 20.0 * std::f32::consts::PI;
            arc_center + Vec2::angled(a) * 10.5 * scale
        })
        .collect();
    let s = Stroke::new(2.2 * scale, c);
    p.add(egui::Shape::line(pts, s));
    p.line_segment([center + egui::vec2(0.0, 7.5 * scale), center + egui::vec2(0.0, 12.0 * scale)], s);
    p.line_segment([center + egui::vec2(-5.5 * scale, 12.5 * scale), center + egui::vec2(5.5 * scale, 12.5 * scale)], s);
}

fn progress_bar(ui: &mut egui::Ui, fraction: f32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 6.0), Sense::hover());
    ui.painter().rect_filled(rect, 3.0, BORDER);
    let mut fill = rect;
    fill.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    ui.painter().rect_filled(fill, 3.0, ACCENT);
}

/// A row of equal-width pill buttons; returns the clicked index.
fn segmented(ui: &mut egui::Ui, labels: &[String], selected: usize) -> Option<usize> {
    let mut clicked = None;
    egui::Frame::none()
        .fill(SURFACE_HI)
        .rounding(10.0)
        .inner_margin(egui::Margin::same(3.0))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            let w = (ui.available_width() - 3.0 * (labels.len() as f32 - 1.0)) / labels.len() as f32;
            ui.horizontal(|ui| {
                for (i, label) in labels.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 28.0), Sense::click());
                    let on = i == selected;
                    if on {
                        ui.painter().rect_filled(rect, 8.0, SURFACE_HOVER);
                        ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.0, BORDER));
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rect, 8.0, with_alpha(SURFACE_HOVER, 0.5));
                    }
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional(13.0),
                        if on { TEXT } else { MUTED },
                    );
                    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        clicked = Some(i);
                    }
                }
            });
        });
    clicked
}

fn section_title(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).strong().size(15.0));
    if !subtitle.is_empty() {
        ui.label(RichText::new(subtitle).small().color(MUTED));
    }
    ui.add_space(4.0);
}

fn accent_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
            .fill(ACCENT)
            .stroke(Stroke::NONE)
            .rounding(9.0),
    )
    .on_hover_cursor(CursorIcon::PointingHand)
}

// -------------------------------------------------------------------- UI sections

impl FlowStateApp {
    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.view == View::Settings {
                if icon_button(ui, 30.0, "Back", false, paint_back).clicked() {
                    self.view = View::Tasks;
                }
                ui.label(RichText::new("Settings").heading().strong());
                return;
            }
            // Logo mark
            let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::hover());
            ui.painter().circle_filled(rect.center(), 10.0, ACCENT);
            ui.painter().circle_filled(rect.center() + egui::vec2(3.0, 3.0), 6.0, with_alpha(ACCENT_2, 0.9));
            ui.label(RichText::new("FlowState").heading().strong());

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_button(ui, 30.0, "Settings", false, paint_gear).clicked() {
                    self.view = View::Settings;
                    self.mic_devices = audio::input_device_names();
                }
                let tip = if self.always_on_top { "Unpin window" } else { "Keep window on top" };
                if icon_button(ui, 30.0, tip, self.always_on_top, paint_pin).clicked() {
                    self.always_on_top = !self.always_on_top;
                    self.save("always_on_top", &self.always_on_top.to_string());
                }
            });
        });
    }

    fn setup_banner(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.speech_engine != SpeechEngine::OnDevice {
            return;
        }
        let Some(spec) = self.selected_speech_model() else { return };
        if spec.is_installed() {
            return;
        }
        theme::card()
            .stroke(Stroke::new(1.0, with_alpha(ACCENT, 0.5)))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                section_title(
                    ui,
                    "Set up voice capture",
                    "FlowState transcribes on your device — nothing leaves your computer.",
                );
                if let Some(progress) = self.downloading(spec.id) {
                    let (done, total) = {
                        let p = progress.lock().unwrap();
                        (p.downloaded, p.total)
                    };
                    ui.label(
                        RichText::new(format!(
                            "Downloading {} · {} of {}",
                            spec.name,
                            models::format_size(done),
                            models::format_size(total)
                        ))
                        .small()
                        .color(MUTED),
                    );
                    progress_bar(ui, done as f32 / total.max(1) as f32, ui.available_width());
                } else if accent_button(
                    ui,
                    &format!("Download {} ({})", spec.name, models::format_size(spec.total_size())),
                )
                .clicked()
                {
                    self.start_download(spec, ctx);
                }
            });
        ui.add_space(4.0);
    }

    fn hero(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::card()
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let time = ui.input(|i| i.time) as f32;

                ui.horizontal(|ui| {
                    // Record button inside the check-in ring.
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(84.0), Sense::hover());
                    let center = rect.center();
                    let ring_r = 38.0;
                    let ring_w = 4.0;
                    let p = ui.painter();
                    p.circle_stroke(center, ring_r, Stroke::new(ring_w, BORDER));

                    let total = self.timer_total().as_secs_f32();
                    if total > 0.0 {
                        let frac = self.timer_remaining().as_secs_f32() / total;
                        let steps = (72.0 * frac).ceil().max(1.0) as usize;
                        let start = -std::f32::consts::FRAC_PI_2;
                        let sweep = std::f32::consts::TAU * frac;
                        let point = |t: f32| center + Vec2::angled(start + sweep * t) * ring_r;
                        let alpha = if self.timer_paused.is_some() { 0.4 } else { 1.0 };
                        for i in 0..steps {
                            let (t0, t1) = (i as f32 / steps as f32, (i + 1) as f32 / steps as f32);
                            let c = with_alpha(lerp_color(ACCENT, ACCENT_2, t1), alpha);
                            p.line_segment([point(t0), point(t1)], Stroke::new(ring_w, c));
                        }
                        if frac > 0.0 {
                            p.circle_filled(point(0.0), ring_w / 2.0, with_alpha(ACCENT, alpha));
                            p.circle_filled(point(1.0), ring_w / 2.0, with_alpha(lerp_color(ACCENT, ACCENT_2, 1.0), alpha));
                        }
                    }

                    let button_r = 29.0;
                    let button_rect = Rect::from_center_size(center, Vec2::splat(button_r * 2.0));
                    let response = ui
                        .interact(button_rect, ui.id().with("record"), Sense::click_and_drag())
                        .on_hover_cursor(if self.busy() { CursorIcon::Wait } else { CursorIcon::PointingHand });
                    let p = ui.painter();
                    let hovered = response.hovered();

                    if let Some(rec) = &self.recording {
                        let level = rec.level();
                        let pulse = (time * 3.0).sin() * 0.5 + 0.5;
                        let r = button_r + 3.0 + level * 6.0 + pulse * 2.0;
                        p.circle_filled(center, r, with_alpha(DANGER, 0.25 * (0.6 + level)));
                        p.circle_filled(center, button_r, DANGER);
                        p.rect_filled(Rect::from_center_size(center, Vec2::splat(14.0)), 3.0, Color32::WHITE);
                    } else if self.busy() {
                        p.circle_filled(center, button_r, SURFACE_HI);
                        let start = time * 4.0;
                        let pts: Vec<egui::Pos2> = (0..=20)
                            .map(|i| center + Vec2::angled(start + i as f32 / 20.0 * 2.2) * 13.0)
                            .collect();
                        p.add(egui::Shape::line(pts, Stroke::new(3.0, ACCENT)));
                    } else {
                        p.circle_filled(center, button_r, if hovered { SURFACE_HOVER } else { SURFACE_HI });
                        p.circle_stroke(center, button_r, Stroke::new(1.5, with_alpha(ACCENT, if hovered { 1.0 } else { 0.6 })));
                        paint_mic(p, center, 0.8, TEXT);
                    }

                    // Press handling: tap toggles, hold is push-to-talk.
                    if response.is_pointer_button_down_on() && self.press.is_none() && !self.busy() {
                        let started_recording = self.recording.is_none() && self.start_recording();
                        self.press = Some(Press { at: Instant::now(), started_recording });
                    }
                    if let Some(press) = &self.press {
                        if !ctx.input(|i| i.pointer.primary_down()) {
                            let held = press.at.elapsed();
                            let started = press.started_recording;
                            self.press = None;
                            if self.recording.is_some() && (!started || held >= HOLD_THRESHOLD) {
                                self.stop_recording(ctx);
                            }
                        }
                    }

                    // Status and timer
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let (title, detail, color) = if let Some(rec) = &self.recording {
                            let held = self.press.as_ref().map(|p| p.started_recording).unwrap_or(false);
                            (
                                format!("Listening  {}", fmt_clock(rec.started.elapsed())),
                                if held { "Release to finish".to_string() } else { "Tap to finish · Esc to discard".to_string() },
                                DANGER,
                            )
                        } else if let Some(job) = &self.job {
                            (job.stage.to_string(), "Working on your note".to_string(), ACCENT)
                        } else if let Some((t, _)) = self
                            .last_transcript
                            .as_ref()
                            .filter(|(_, at)| at.elapsed() < Duration::from_secs(12))
                        {
                            ("Captured".to_string(), format!("“{t}”"), SUCCESS)
                        } else {
                            ("Tap to talk".to_string(), "Hold for push-to-talk · Space".to_string(), TEXT)
                        };
                        ui.label(RichText::new(title).strong().color(color));
                        ui.add(egui::Label::new(RichText::new(detail).small().color(MUTED)).truncate());

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            if self.timer_mins == 0 {
                                ui.label(RichText::new("Check-in timer off").small().color(FAINT));
                                return;
                            }
                            let label = if self.timer_paused.is_some() { "Paused" } else { "Check-in" };
                            ui.label(RichText::new(label).small().color(MUTED));
                            ui.label(RichText::new(fmt_clock(self.timer_remaining())).strong());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if icon_button(ui, 24.0, "Restart timer", false, paint_reset).clicked() {
                                    self.reset_timer();
                                }
                                let paused = self.timer_paused.is_some();
                                let (tip, paint): (&str, fn(&egui::Painter, Rect, Color32)) =
                                    if paused { ("Resume", paint_play) } else { ("Pause", paint_pause) };
                                if icon_button(ui, 24.0, tip, false, paint).clicked() {
                                    self.toggle_timer_pause();
                                }
                            });
                        });
                    });
                });
            });
    }

    fn composer(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(SURFACE)
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(14.0)
            .inner_margin(egui::Margin { left: 12.0, right: 5.0, top: 5.0, bottom: 5.0 })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let input = ui.add(
                        egui::TextEdit::singleline(&mut self.new_task_text)
                            .hint_text(RichText::new("Add a task…").color(FAINT))
                            .frame(false)
                            .char_limit(MAX_TASK_LEN)
                            .vertical_align(Align::Center)
                            .margin(egui::vec2(0.0, 4.0))
                            .min_size(egui::vec2(0.0, 32.0))
                            .desired_width(ui.available_width() - 32.0 - ui.spacing().item_spacing.x),
                    );
                    if self.focus_composer {
                        input.request_focus();
                        self.focus_composer = false;
                    }
                    let submit = input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    let enabled = !self.new_task_text.trim().is_empty();
                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
                    let fill = if !enabled {
                        SURFACE_HI
                    } else if resp.hovered() {
                        lerp_color(ACCENT, Color32::WHITE, 0.12)
                    } else {
                        ACCENT
                    };
                    ui.painter().rect_filled(rect, 10.0, fill);
                    paint_plus(ui.painter(), rect.shrink(10.0), if enabled { Color32::WHITE } else { FAINT });
                    let clicked = resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Add task").clicked();

                    if enabled && (submit || clicked) {
                        self.add_typed_task();
                    }
                });
            });
    }

    fn tabs(&mut self, ui: &mut egui::Ui) {
        let active = self.tasks.iter().filter(|t| !t.completed).count();
        let done = self.tasks.len() - active;
        let labels = [format!("To do  {active}"), format!("Done  {done}")];
        let selected = if self.tab == Tab::Active { 0 } else { 1 };
        if let Some(i) = segmented(ui, &labels, selected) {
            self.tab = if i == 0 { Tab::Active } else { Tab::Done };
            self.commit_edit();
        }
    }

    fn task_list(&mut self, ui: &mut egui::Ui) {
        let show_done = self.tab == Tab::Done;
        let visible: Vec<Task> = self
            .tasks
            .iter()
            .filter(|t| t.completed == show_done)
            .cloned()
            .collect();

        if visible.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                let (title, body) = if show_done {
                    ("Nothing finished yet", "Completed tasks from the last 7 days show up here.")
                } else {
                    ("You're all clear", "Tap the mic and talk through what's next, or type a task above.")
                };
                ui.label(RichText::new(title).strong().size(15.0));
                ui.label(RichText::new(body).color(MUTED));
            });
            return;
        }

        let mut toggle = None;
        let mut delete = None;
        let mut start_edit = None;
        let mut commit = false;
        let mut cancel = false;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                for task in &visible {
                    let row_id = ui.id().with(("task_row", task.id));
                    // Hover is judged against last frame's row rect so the background can be painted first.
                    let row_hovered = ui
                        .ctx()
                        .data(|d| d.get_temp::<Rect>(row_id))
                        .map(|r| ui.rect_contains_pointer(r))
                        .unwrap_or(false);
                    let editing_this = matches!(&self.editing, Some((id, _)) if *id == task.id);

                    let bg = ui.painter().add(egui::Shape::Noop);
                    let row = egui::Frame::none()
                        .inner_margin(egui::Margin::symmetric(8.0, 5.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;

                                // Round checkbox
                                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                                let c = rect.center();
                                if task.completed {
                                    ui.painter().circle_filled(c, 9.0, SUCCESS);
                                    paint_check(ui.painter(), Rect::from_center_size(c, Vec2::splat(8.0)), BG, 2.0);
                                } else {
                                    let stroke = if resp.hovered() { ACCENT } else { FAINT };
                                    ui.painter().circle_stroke(c, 8.0, Stroke::new(1.6, stroke));
                                    if resp.hovered() {
                                        paint_check(ui.painter(), Rect::from_center_size(c, Vec2::splat(7.0)), with_alpha(ACCENT, 0.7), 1.6);
                                    }
                                }
                                let tip = if task.completed { "Mark as not done" } else { "Mark as done" };
                                if resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip).clicked() {
                                    toggle = Some(task.id);
                                }

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    // Delete (visible on hover)
                                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                                    if row_hovered || resp.hovered() {
                                        if resp.hovered() {
                                            ui.painter().rect_filled(rect, 6.0, with_alpha(DANGER, 0.15));
                                        }
                                        paint_trash(ui.painter(), rect.shrink(5.5), if resp.hovered() { DANGER } else { MUTED });
                                    }
                                    if resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Delete").clicked() {
                                        delete = Some(task.id);
                                    }

                                    // Age, inline on the right
                                    let when = match (&task.completed_at, task.completed) {
                                        (Some(at), true) => relative_time(at),
                                        _ => relative_time(&task.created_at),
                                    };
                                    ui.label(RichText::new(when).small().color(FAINT));

                                    // Text (or editor) takes the remaining width
                                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                        if editing_this {
                                            let (_, buf) = self.editing.as_mut().unwrap();
                                            let edit = ui.add(
                                                egui::TextEdit::multiline(buf)
                                                    .desired_rows(1)
                                                    .char_limit(MAX_TASK_LEN)
                                                    .margin(egui::vec2(4.0, 2.0))
                                                    .desired_width(ui.available_width() - 8.0)
                                                    .return_key(None),
                                            )
                                            .on_hover_text("Enter to save · Esc to cancel");
                                            if self.focus_editor {
                                                edit.request_focus();
                                                self.focus_editor = false;
                                            }
                                            let (enter, esc) = ui.input(|i| {
                                                (
                                                    i.key_pressed(egui::Key::Enter) && !i.modifiers.shift,
                                                    i.key_pressed(egui::Key::Escape),
                                                )
                                            });
                                            if esc {
                                                cancel = true;
                                            } else if (edit.has_focus() && enter) || edit.lost_focus() {
                                                commit = true;
                                            }
                                        } else {
                                            let text = if task.completed {
                                                RichText::new(&task.text).strikethrough().color(MUTED)
                                            } else {
                                                RichText::new(&task.text).color(TEXT)
                                            };
                                            let label = ui
                                                .add(egui::Label::new(text).wrap().sense(Sense::click()))
                                                .on_hover_cursor(CursorIcon::Text)
                                                .on_hover_text("Click to edit");
                                            if label.clicked() {
                                                start_edit = Some((task.id, task.text.clone()));
                                            }
                                        }
                                    });
                                });
                            });
                        })
                        .response;

                    ui.ctx().data_mut(|d| d.insert_temp(row_id, row.rect));
                    let (fill, stroke) = if editing_this {
                        (SURFACE_HI, with_alpha(ACCENT, 0.7))
                    } else if row_hovered {
                        (SURFACE_HI, BORDER)
                    } else {
                        (SURFACE, with_alpha(BORDER, 0.6))
                    };
                    ui.painter().set(
                        bg,
                        egui::epaint::RectShape::new(row.rect, 10.0, fill, Stroke::new(1.0, stroke)),
                    );
                }

                if show_done {
                    ui.add_space(4.0);
                    ui.vertical_centered(|ui| {
                        if ui
                            .add(egui::Button::new(RichText::new("Clear completed").small().color(MUTED)).frame(false))
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .clicked()
                        {
                            for t in &visible {
                                let _ = database::delete_task(&self.db, t.id);
                            }
                            self.reload_tasks();
                        }
                    });
                }
            });

        if cancel {
            self.editing = None;
        } else if commit {
            self.commit_edit();
        }
        if let Some((id, text)) = start_edit {
            self.commit_edit();
            self.editing = Some((id, text));
            self.focus_editor = true;
        }
        if let Some(id) = toggle {
            let _ = database::toggle_task(&self.db, id);
            self.reload_tasks();
        }
        if let Some(id) = delete {
            if matches!(self.editing, Some((e, _)) if e == id) {
                self.editing = None;
            }
            let _ = database::delete_task(&self.db, id);
            self.reload_tasks();
        }
    }

    fn model_rows(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, kind: ModelKind) {
        let selected_id = match kind {
            ModelKind::Speech => self.speech_model.clone(),
            ModelKind::Language => self.language_model.clone(),
        };
        for spec in models::of_kind(kind) {
            let installed = spec.is_installed();
            let selected = spec.id == selected_id;
            let progress = self.downloading(spec.id);

            let frame = egui::Frame::none()
                .fill(SURFACE_HI)
                .stroke(Stroke::new(1.0, if selected && installed { with_alpha(ACCENT, 0.7) } else { BORDER }))
                .rounding(10.0)
                .inner_margin(egui::Margin::symmetric(10.0, 8.0));
            frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                let row_size = egui::vec2(ui.available_width(), 34.0);
                ui.allocate_ui_with_layout(row_size, Layout::right_to_left(Align::Center), |ui| {
                    {
                        if let Some(p) = &progress {
                            let (done, total) = {
                                let p = p.lock().unwrap();
                                (p.downloaded, p.total)
                            };
                            let frac = done as f32 / total.max(1) as f32;
                            ui.label(RichText::new(format!("{:.0}%", frac * 100.0)).small().color(MUTED));
                            progress_bar(ui, frac, 70.0);
                        } else if installed {
                            if !self.busy()
                                && icon_button(ui, 26.0, "Delete model", false, paint_trash).clicked()
                            {
                                match spec.remove() {
                                    Ok(()) => self.toast(ToastKind::Info, format!("{} removed", spec.name)),
                                    Err(e) => self.toast(ToastKind::Error, format!("Couldn't remove: {e}")),
                                }
                            }
                            if selected {
                                ui.label(RichText::new("✔ In use").small().color(SUCCESS));
                            } else if ui.button("Use").clicked() {
                                match kind {
                                    ModelKind::Speech => {
                                        self.speech_model = spec.id.into();
                                        self.save("speech_model", spec.id);
                                    }
                                    ModelKind::Language => {
                                        self.language_model = spec.id.into();
                                        self.save("language_model", spec.id);
                                        self.warm_up_language_model();
                                    }
                                }
                            }
                        } else if accent_button(ui, "Download").clicked() {
                            self.start_download(spec, ctx);
                        }
                    }
                    // Name and description take whatever width is left.
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!("{}  ·  {}", spec.name, models::format_size(spec.total_size()))).strong(),
                            )
                            .truncate(),
                        );
                        ui.add(egui::Label::new(RichText::new(spec.blurb).small().color(MUTED)).truncate());
                    });
                });
            });
        }
    }

    fn settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;

                theme::card().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    section_title(ui, "Check-in timer", "A gentle nudge to capture what you're doing.");
                    let slider = ui.add(
                        egui::Slider::new(&mut self.timer_mins, 0..=90)
                            .suffix(" min")
                            .custom_formatter(|v, _| if v == 0.0 { "Off".into() } else { format!("{v:.0}") }),
                    );
                    if slider.changed() {
                        self.timer_paused = None;
                        self.reset_timer();
                        self.save("timer_mins", &self.timer_mins.to_string());
                    }
                    if ui.checkbox(&mut self.chime, "Play a chime").changed() {
                        self.save("chime", &self.chime.to_string());
                    }
                });

                theme::card().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    section_title(ui, "Microphone", "");
                    let current = self.mic_device.clone().unwrap_or_else(|| "System default".into());
                    let mut changed = false;
                    egui::ComboBox::from_id_salt("mic")
                        .selected_text(current)
                        .width(ui.available_width() - 8.0)
                        .show_ui(ui, |ui| {
                            changed |= ui.selectable_value(&mut self.mic_device, None, "System default").changed();
                            for name in self.mic_devices.clone() {
                                changed |= ui
                                    .selectable_value(&mut self.mic_device, Some(name.clone()), name)
                                    .changed();
                            }
                        });
                    if changed {
                        self.save("mic_device", self.mic_device.as_deref().unwrap_or(""));
                    }
                });

                theme::card().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    section_title(ui, "Speech recognition", "Whisper turns your voice into text.");
                    let labels = ["On this device".to_string(), "Remote server".to_string()];
                    let sel = if self.speech_engine == SpeechEngine::OnDevice { 0 } else { 1 };
                    if let Some(i) = segmented(ui, &labels, sel) {
                        self.speech_engine = if i == 0 { SpeechEngine::OnDevice } else { SpeechEngine::Remote };
                        self.save("speech_engine", if i == 0 { "builtin" } else { "remote" });
                    }
                    match self.speech_engine {
                        SpeechEngine::OnDevice => self.model_rows(ui, ctx, ModelKind::Speech),
                        SpeechEngine::Remote => {
                            ui.label(RichText::new("whisper.cpp server endpoint").small().color(MUTED));
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut self.whisper_url)
                                    .hint_text("http://192.168.1.10:8080/inference")
                                    .desired_width(f32::INFINITY),
                            );
                            if edit.lost_focus() || edit.changed() {
                                self.save("whisper_url", self.whisper_url.trim());
                            }
                        }
                    }
                });

                theme::card().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    section_title(ui, "Task understanding", "How spoken notes become separate tasks.");
                    let engines = [ParseEngine::Rules, ParseEngine::BuiltIn, ParseEngine::Ollama];
                    let labels = ["Quick rules".to_string(), "On-device AI".to_string(), "Ollama".to_string()];
                    let sel = engines.iter().position(|e| *e == self.parse_engine).unwrap_or(0);
                    if let Some(i) = segmented(ui, &labels, sel) {
                        self.parse_engine = engines[i];
                        self.save("parser", engines[i].as_str());
                        self.warm_up_language_model();
                    }
                    match self.parse_engine {
                        ParseEngine::Rules => {
                            ui.label(RichText::new("Instant keyword matching. Splits lists on commas and “and”.").small().color(MUTED));
                        }
                        ParseEngine::BuiltIn => {
                            ui.label(RichText::new("A small language model running locally on your CPU. Better at splitting and matching finished tasks.").small().color(MUTED));
                            self.model_rows(ui, ctx, ModelKind::Language);
                        }
                        ParseEngine::Ollama => {
                            ui.label(RichText::new("Server URL").small().color(MUTED));
                            let a = ui.add(egui::TextEdit::singleline(&mut self.ollama_url).desired_width(f32::INFINITY));
                            ui.label(RichText::new("Model").small().color(MUTED));
                            let b = ui.add(egui::TextEdit::singleline(&mut self.ollama_model).desired_width(f32::INFINITY));
                            if a.changed() || b.changed() {
                                self.save("ollama_url", self.ollama_url.trim());
                                self.save("ollama_model", self.ollama_model.trim());
                            }
                        }
                    }
                });

                theme::card().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    section_title(ui, "Window", "");
                    if ui.checkbox(&mut self.always_on_top, "Keep FlowState above other windows").changed() {
                        self.save("always_on_top", &self.always_on_top.to_string());
                    }
                });

                theme::card().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    section_title(ui, "Storage", "Tasks and models are stored locally.");
                    let dir = models::data_dir();
                    ui.label(RichText::new(dir.display().to_string()).small().monospace().color(MUTED));
                    if ui.button("Open folder").clicked() {
                        let _ = std::fs::create_dir_all(&dir);
                        #[cfg(windows)]
                        let opener = "explorer";
                        #[cfg(not(windows))]
                        let opener = "xdg-open";
                        if let Err(e) = std::process::Command::new(opener).arg(&dir).spawn() {
                            self.toast(ToastKind::Error, format!("Couldn't open folder: {e}"));
                        }
                    }
                });

                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(format!("FlowState {}", env!("CARGO_PKG_VERSION"))).small().color(FAINT));
                });
            });
    }

    fn toasts(&mut self, ctx: &egui::Context) {
        self.toasts.retain(|t| {
            let ttl = if t.kind == ToastKind::Error { 7.0 } else { 4.0 };
            t.born.elapsed().as_secs_f32() < ttl
        });
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(Align2::CENTER_BOTTOM, egui::vec2(0.0, -14.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                let width = (ctx.screen_rect().width() - 32.0).min(360.0);
                ui.set_max_width(width);
                for t in &self.toasts {
                    let ttl = if t.kind == ToastKind::Error { 7.0 } else { 4.0 };
                    let age = t.born.elapsed().as_secs_f32();
                    let fade = ((ttl - age) / 0.4).clamp(0.0, 1.0) * (age / 0.15).clamp(0.0, 1.0);
                    let color = match t.kind {
                        ToastKind::Info => ACCENT_2,
                        ToastKind::Success => SUCCESS,
                        ToastKind::Error => DANGER,
                    };
                    egui::Frame::none()
                        .fill(with_alpha(SURFACE_HOVER, 0.97 * fade))
                        .stroke(Stroke::new(1.0, with_alpha(color, 0.5 * fade)))
                        .rounding(12.0)
                        .inner_margin(egui::Margin::symmetric(12.0, 9.0))
                        .shadow(egui::epaint::Shadow {
                            offset: egui::vec2(0.0, 6.0),
                            blur: 18.0,
                            spread: 0.0,
                            color: Color32::from_black_alpha((110.0 * fade) as u8),
                        })
                        .show(ui, |ui| {
                            ui.set_width(width - 24.0);
                            ui.horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 16.0), Sense::hover());
                                ui.painter().circle_filled(r.center(), 4.0, with_alpha(color, fade));
                                ui.add(egui::Label::new(RichText::new(&t.text).color(with_alpha(TEXT, fade))).wrap());
                            });
                        });
                    ui.add_space(6.0);
                }
            });
    }
}

impl eframe::App for FlowStateApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::from(BG).to_array()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_background();
        self.tick_timer();

        if self.applied_on_top != Some(self.always_on_top) {
            let level = if self.always_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
            self.applied_on_top = Some(self.always_on_top);
        }

        // Keyboard: Space toggles recording when no text field is focused; Esc discards.
        let typing = ctx.memory(|m| m.focused().is_some());
        let (space, esc) = ctx.input(|i| (i.key_pressed(egui::Key::Space), i.key_pressed(egui::Key::Escape)));
        if self.view == View::Tasks && !typing && space && self.press.is_none() && !self.busy() {
            if self.recording.is_some() {
                self.stop_recording(ctx);
            } else {
                self.start_recording();
            }
        }
        if esc && self.recording.is_some() {
            self.cancel_recording();
        }

        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin { left: 16.0, right: 12.0, top: 12.0, bottom: 4.0 }))
            .show_separator_line(false)
            .show(ctx, |ui| self.header(ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin { left: 14.0, right: 14.0, top: 8.0, bottom: 10.0 }))
            .show(ctx, |ui| match self.view {
                View::Settings => self.settings(ui, ctx),
                View::Tasks => {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    self.setup_banner(ui, ctx);
                    self.hero(ui, ctx);
                    self.composer(ui);
                    self.tabs(ui);
                    self.task_list(ui);
                }
            });

        self.toasts(ctx);

        // Repaint only as often as needed: smooth while animating, once a second when idle.
        if self.recording.is_some() || self.busy() || !self.toasts.is_empty() || !self.downloads.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(33));
        } else {
            let into_second = self.timer_started.elapsed().subsec_millis() as u64;
            ctx.request_repaint_after(Duration::from_millis(1000 - into_second.min(999)));
        }
    }
}
