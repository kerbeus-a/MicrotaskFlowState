# External Integrations

**Analysis Date:** 2026-01-22

## APIs & External Services

**Ollama LLM (Optional):**
- Service: Ollama - Local LLM for transcript parsing
- What it's used for: Natural language processing of voice recordings to extract tasks with actions (add/complete/remove)
- SDK/Client: `reqwest` HTTP client (Rust)
- Endpoints:
  - `{OLLAMA_URL}/api/tags` - List available models
  - `{OLLAMA_URL}/api/generate` - Generate text completions
- Auth: None (localhost connection assumed)
- Configuration:
  - `OLLAMA_URL` env var (defaults to `http://localhost:11434`)
  - `OLLAMA_MODEL` env var (defaults to `llama3.2`)
- Status: Optional fallback - app works without it in "fast mode"
- Implementation: `src-tauri/src/ollama.rs`

## Data Storage

**Databases:**
- SQLite (bundled via `rusqlite`)
  - Location: Platform app data directory (`%APPDATA%\flowstate\` on Windows)
  - Database file: `flowstate.db`
  - Client: `rusqlite 0.30` with bundled SQLite
  - Tables:
    - `tasks` - Task list with completion status and timestamps
    - `settings` - Key-value store (e.g., `ollama_enabled` flag)

**File Storage:**
- Local filesystem only
  - Audio files: WAV format, stored in app data directory
  - Whisper models: Stored in app data directory as `.bin` files
  - App data accessed via `dirs` crate (Windows: `%APPDATA%\flowstate\`)

**Caching:**
- In-memory Whisper model cache (`WhisperCache` struct in `src-tauri/src/whisper.rs`)
  - Prevents reloading speech-to-text model on every recording
  - Thread-safe using `Mutex<Option<(WhisperModelSize, Arc<WhisperContext>)>>`

## Authentication & Identity

**Auth Provider:**
- None - Desktop application with no user accounts
- All data is local and user-specific

**Security:**
- No API keys or external authentication required
- Tauri handles sandboxing and IPC validation

## Monitoring & Observability

**Error Tracking:**
- None (no external service)

**Logs:**
- Console output via `eprintln!` macros in Rust
- Logs include operation status (✅ success, ⚠️ warnings, 🔄 processing)
- No persistent logging to file or external service

## CI/CD & Deployment

**Hosting:**
- None - Desktop application only
- Distributed as standalone Windows executable via Tauri

**CI Pipeline:**
- None configured (not detected in codebase)

## Environment Configuration

**Required env vars:**
- `OLLAMA_URL` (optional, defaults to `http://localhost:11434`)
- `OLLAMA_MODEL` (optional, defaults to `llama3.2`)

**Secrets location:**
- No secrets management - application is local-only
- No `.env` file detected in codebase

## Webhooks & Callbacks

**Incoming:**
- None - No incoming webhooks

**Outgoing:**
- None - No outgoing webhooks to external services

## Window/IPC Communication

**Tauri Commands (Frontend ↔ Backend):**
The following Tauri commands bridge React frontend to Rust backend via `invoke()`:

**Task Management:**
- `get_tasks()` - Fetch all non-deleted tasks
- `add_task(text)` - Create new task
- `update_task(id, text)` - Modify task text
- `delete_task(id)` - Delete task
- `toggle_task(id)` - Mark task complete/incomplete

**Voice Recording:**
- `transcribe_audio(audio_path, model_size)` - Convert WAV to text via local Whisper
- `process_voice_recording(transcript)` - Parse transcript and extract task actions
- `process_voice_log(transcript)` - Full pipeline: parse → update database → return results

**Whisper Model Management:**
- `list_whisper_models()` - Show available model sizes
- `download_whisper_model(model_size)` - Download Whisper model binary
- `check_whisper_model(model_size)` - Verify model exists
- `delete_whisper_model(model_size)` - Remove downloaded model
- `save_audio_file(audio_data)` - Store WAV recording

**Timer:**
- `get_timer_status()` - Current timer state
- `reset_timer()` - Reset to configured duration
- `get_timer_duration()` - Get configured duration in minutes
- `set_timer_duration(minutes)` - Update timer duration

**Window State:**
- `get_window_state()` - Restore position/size/state
- `set_window_state(state)` - Update pinned state
- `save_window_state(x, y, width, height, pinned)` - Persist window layout
- `load_window_state()` - Retrieve saved layout
- `set_always_on_top(enabled)` - Toggle always-on-top

**Settings:**
- `get_ollama_enabled()` - Check if LLM parsing is enabled
- `set_ollama_enabled(enabled)` - Toggle Ollama usage
- `get_autostart_enabled()` - Check if app autostart is enabled
- `set_autostart_enabled(enabled)` - Toggle autostart (Windows registry)

## Tauri Events

**Emitted to Frontend:**
- Window events (close, focus, etc.) - handled in `src-tauri/src/main.rs`
- Custom timer/awareness events - implemented in `src-tauri/src/timer.rs`

## Model Management & Downloads

**Whisper Models:**
- Models downloaded from Hugging Face (inferred from whisper-rs defaults)
- Available sizes: Tiny (75MB), Base (142MB), Small (466MB), Medium (1.4GB), Large (2.9GB)
- Implementation: `src-tauri/src/whisper.rs` (download, check, delete functions)

---

*Integration audit: 2026-01-22*
