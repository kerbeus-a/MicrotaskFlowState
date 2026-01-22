# Architecture

**Analysis Date:** 2026-01-22

## Pattern Overview

**Overall:** Desktop Application with Tauri Desktop Framework + React Frontend + Rust Backend

**Key Characteristics:**
- Client-server IPC architecture using Tauri's invoke/listen system
- Clear separation between React UI layer and Rust backend
- Event-driven communication for real-time updates (timer alerts, recording triggers)
- State management primarily in React hooks with backend persistence via SQLite
- Modular Rust backend with domain-specific modules (database, timer, audio processing)

## Layers

**Frontend Layer (React + TypeScript):**
- Purpose: User interface and interaction handling, local state management for UI state only
- Location: `src/`
- Contains: React components, hooks, TypeScript interfaces, CSS styling
- Depends on: Tauri API (`@tauri-apps/api`), Web Audio API for recording
- Used by: Electron window (Tauri webview)
- Key files: `src/App.tsx` (main component), `src/main.tsx` (entry point), `src/components/`, `src/hooks/`

**Tauri Desktop Framework Layer:**
- Purpose: Bridge between frontend and backend, window management, system integration
- Location: Tauri configuration and native layer
- Contains: Window event handling, plugin initialization, command invocation
- Depends on: Tauri plugins (dialog, shell, global-shortcut)
- Used by: Both frontend and backend

**Backend Service Layer (Rust):**
- Purpose: Business logic, system access, data persistence, audio processing
- Location: `src-tauri/src/`
- Contains: Command handlers, service modules
- Modules:
  - **commands.rs**: Tauri command handlers that serve as API endpoints
  - **database.rs**: SQLite database operations and task management
  - **timer.rs**: Awareness timer with configurable duration and alert system
  - **whisper.rs**: Speech-to-text model management and transcription
  - **ollama.rs**: Local LLM integration for task extraction
  - **main.rs**: Application entry point, service initialization

**Data Layer:**
- Purpose: Persistent storage and configuration
- Location: SQLite database at `%APPDATA%/flowstate/flowstate.db`
- Contains: Tasks table with timestamps, settings table for preferences
- Indexes: `idx_tasks_completed` for efficient filtering

## Data Flow

**Voice Recording Flow:**

1. User holds record button in `RecordButton.tsx`
2. `handleStartRecording()` calls `useAudioRecorder.startRecording()`
3. `useAudioRecorder` requests microphone access via Web Audio API
4. MediaRecorder captures audio as WebM Opus
5. Audio is converted to WAV format via `WavEncoder` class
6. WAV blob is sent to backend via `invoke("process_voice_recording")`
7. Backend's `process_voice_recording` command (in `commands.rs`) receives audio bytes
8. `whisper.rs` loads WhisperContext and transcribes audio
9. Transcript sent to `ollama.rs` which parses it into task actions
10. Tasks added/completed in database via `database.rs`
11. Frontend calls `loadTasks()` via `invoke("get_tasks")` to refresh UI

**Timer Alert Flow:**

1. Backend timer (in `timer.rs`) checks remaining time every 10 seconds
2. When timer expires, backend emits "timer-alert" event via Tauri
3. Frontend listens via `listen("timer-alert")` in `App.tsx`
4. Frontend plays chime sound and syncs timer state with backend
5. Backend resets timer and saves state

**Window State Persistence:**

1. On app startup, `App.tsx` calls `invoke("load_window_state")`
2. On window move/resize, debounced handler calls `invoke("save_window_state")`
3. Backend stores window geometry in database or config file

**State Management:**

- **Frontend Local State:** Recording state, UI visibility (modals), timer display values - managed in React hooks
- **Backend State:** Timer start time and duration (static Mutex), database records - persisted via SQLite
- **Cross-Process State:** Window position, timer duration preference - synced via Tauri IPC

## Key Abstractions

**Task Entity:**
- Purpose: Core business model representing user-created actions
- Examples: `src/App.tsx` (line 17-23), `src-tauri/src/database.rs` (line 11-18), `src-tauri/src/commands.rs` (line 6-13)
- Pattern: Serialized across IPC boundary, stored in SQLite, displayed in TaskList component
- Properties: id, text, completed flag, created_at, completed_at timestamps

**WhisperCache:**
- Purpose: Thread-safe model caching to avoid reloading speech-to-text model on every recording
- Examples: `src-tauri/src/whisper.rs` (line 16-79), `src-tauri/src/main.rs` (line 23-24)
- Pattern: Singleton managed by Tauri application state
- Mechanism: Mutex wraps Arc<WhisperContext>, recovers from poisoned locks

**AudioRecorderHook:**
- Purpose: Encapsulate Web Audio API complexity for React components
- Examples: `src/hooks/useAudioRecorder.ts` (entire file)
- Pattern: Custom React hook returning recorder state and controls
- Features: Device enumeration, audio level monitoring, WAV encoding

**Database:**
- Purpose: Centralized access to SQLite with thread-safe connection pooling
- Examples: `src-tauri/src/database.rs` (line 7-39)
- Pattern: Struct wrapping Mutex<Connection>, passed to Tauri commands via State
- Responsibility: Schema initialization, query execution, error handling

**TaskAction Enum:**
- Purpose: Represent parsed voice commands for task manipulation
- Examples: `src-tauri/src/ollama.rs` (line 38-42)
- Pattern: Semantic task operations extracted from natural language
- Variants: Add(text), Complete(text), Remove(text)

## Entry Points

**Frontend Entry Point:**
- Location: `src/main.tsx`
- Triggers: Browser loads `index.html`, vite dev server or production build
- Responsibilities: Mount React app to DOM root element
- Call chain: main.tsx → App.tsx → component tree

**Backend Entry Point:**
- Location: `src-tauri/src/main.rs`
- Triggers: `npm run tauri dev` or executable launch
- Responsibilities: Initialize Tauri builder, setup plugins, register command handlers, spawn background services
- Setup sequence:
  1. Initialize database and create tables
  2. Create WhisperCache singleton
  3. Setup awareness timer background thread
  4. Register all Tauri commands
  5. Configure window event handlers (prevent close, minimize to tray)

**App Initialization Sequence (App.tsx):**
- Location: `src/App.tsx` lines 42-139 (useEffect on mount)
- Responsibilities: Load initial data, setup listeners, restore window state
- Sequence:
  1. Restore window position/size from backend
  2. Load tasks via `invoke("get_tasks")`
  3. Check installed Whisper models
  4. Sync timer state
  5. Setup interval for local timer countdown
  6. Listen for "timer-alert" events
  7. Listen for "start-recording" global shortcut events

## Error Handling

**Strategy:** Layered error handling with user-friendly fallback messages

**Patterns:**

1. **Tauri Command Errors (Backend):**
   - Example: `src-tauri/src/commands.rs` (line 16-27)
   - Commands return `Result<T, String>`
   - Errors converted to strings with `map_err(|e| e.to_string())`
   - Sent back to frontend as Tauri error

2. **Frontend Promise Errors:**
   - Example: `src/App.tsx` (line 162-170)
   - Try/catch around `invoke()` calls
   - Specific error messages for Ollama, audio decoding, microphone access
   - Messages preserved in state (`processingError`) and displayed to user

3. **Audio Recording Errors (useAudioRecorder.ts):**
   - Lines 368-383: Catch getUserMedia failures
   - Lines 414-418: Catch audio decoding errors
   - Permission errors get specific guidance messages
   - NotFoundError suggests checking microphone selection

4. **Backend Lock Poisoning (whisper.rs):**
   - Lines 32-37: Recover from poisoned Mutex via `unwrap_or_else`
   - Clears cache on panic recovery to ensure consistency

## Cross-Cutting Concerns

**Logging:**
- Frontend: `console.error()`, `console.warn()`, `console.log()` to browser dev tools
- Backend: `eprintln!()` to stderr for development feedback
- Example: `src-tauri/src/whisper.rs` (lines 57, 67, 78) show progress messages

**Validation:**
- Frontend: Input validation in components (e.g., `src/components/TaskList.tsx` line 29 checks `editText.trim()`)
- Backend: Task text validated as non-empty strings before database insertion
- Audio: Sample rate validation and buffer size checks in `useAudioRecorder.ts`

**Authentication:**
- Not implemented - assumes single-user desktop app
- Window focus restoration allows user re-engagement (line 94 in main.rs)

**Configuration:**
- Timer duration persisted to `timer_config.json` in app data directory
- Window state stored and loaded between sessions
- Whisper model selection stored in component state and sent with each recording command
- Ollama enabled flag stored in database settings table

**Resource Management:**
- Audio context cleanup: `src/hooks/useAudioRecorder.ts` lines 435-439
- Stream track cleanup: line 435
- Database connection wrapped in Mutex for thread-safe access
- Timer background task managed by Tauri async runtime

---

*Architecture analysis: 2026-01-22*
