# FlowState Project Structure

## Overview

FlowState is a Tauri-based desktop application with a React frontend and Rust backend.

## Directory Structure

```
MicroTask/
├── src/                          # React frontend (TypeScript)
│   ├── components/               # React components
│   │   ├── TaskList.tsx         # Task list display and management
│   │   ├── RecordButton.tsx     # Voice recording button
│   │   └── TimerBar.tsx         # 15-minute awareness timer display
│   ├── App.tsx                   # Main application component
│   ├── main.tsx                  # React entry point
│   └── styles.css                # Global styles
│
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── main.rs              # Tauri application entry point
│   │   ├── commands.rs          # Tauri command handlers (API endpoints)
│   │   ├── database.rs          # SQLite database operations
│   │   ├── timer.rs             # Awareness timer logic
│   │   ├── ollama.rs            # Ollama LLM integration
│   │   └── whisper.rs           # Whisper.cpp integration (placeholder)
│   ├── Cargo.toml               # Rust dependencies
│   ├── tauri.conf.json          # Tauri configuration
│   ├── build.rs                 # Build script
│   └── icons/                   # Application icons (to be added)
│
├── package.json                  # Node.js dependencies and scripts
├── tsconfig.json                # TypeScript configuration
├── vite.config.ts               # Vite build configuration
├── index.html                   # HTML entry point
├── README.md                    # Main project documentation
├── SETUP.md                     # Detailed setup instructions
├── QUICKSTART.md                # Quick start guide
└── .gitignore                   # Git ignore rules

```

## Key Files

### Frontend (React/TypeScript)

- **`src/App.tsx`**: Main application component that orchestrates all features
- **`src/components/TaskList.tsx`**: Displays and manages the task list
- **`src/components/RecordButton.tsx`**: Voice recording button with visual feedback
- **`src/components/TimerBar.tsx`**: Displays the 15-minute awareness timer

### Backend (Rust)

- **`src-tauri/src/main.rs`**: 
  - Initializes the Tauri application
  - Sets up global shortcuts (Win + Alt + R)
  - Configures window behavior
  - Registers command handlers

- **`src-tauri/src/commands.rs`**: 
  - Tauri command handlers exposed to frontend
  - Functions: `get_tasks`, `add_task`, `update_task`, `delete_task`, `toggle_task`, `process_voice_log`, etc.

- **`src-tauri/src/database.rs`**: 
  - SQLite database initialization
  - CRUD operations for tasks
  - Task querying and filtering

- **`src-tauri/src/timer.rs`**: 
  - 15-minute awareness timer implementation
  - Timer reset and status checking
  - Alert triggering

- **`src-tauri/src/ollama.rs`**: 
  - Integration with Ollama local LLM
  - Transcript parsing to extract tasks
  - JSON response parsing

- **`src-tauri/src/whisper.rs`**: 
  - Placeholder for Whisper.cpp integration
  - Will handle speech-to-text conversion

## Data Flow

1. **Voice Recording** (to be implemented):
   - User clicks record button or presses Win+Alt+R
   - Audio is captured
   - Audio is sent to Whisper.cpp for transcription
   - Transcript is sent to Ollama for parsing
   - Parsed tasks are saved to database
   - UI is updated

2. **Task Management**:
   - Frontend calls Tauri commands
   - Commands interact with SQLite database
   - Results are returned to frontend
   - UI updates reactively

3. **Timer**:
   - Timer starts when app launches
   - Counts down from 15 minutes
   - When expired, triggers alert event
   - Frontend receives event and plays chime
   - Timer resets automatically

## Configuration

### Tauri Configuration (`src-tauri/tauri.conf.json`)
- Window size: 300x600px
- Global shortcuts enabled
- System tray enabled
- Always-on-top option

### Environment Variables
- `OLLAMA_URL`: Ollama server URL (default: http://localhost:11434)
- `OLLAMA_MODEL`: Model name (default: llama3)

## Database Schema

```sql
CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    text TEXT NOT NULL,
    completed INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT
);
```

## API Commands (Tauri)

All commands are async and can be called from the frontend using `invoke()`:

- `get_tasks()` → `Task[]`
- `add_task(text: string)` → `Task`
- `update_task(id: number, text: string)` → `void`
- `delete_task(id: number)` → `void`
- `toggle_task(id: number)` → `Task`
- `process_voice_log(transcript: string)` → `Task[]`
- `get_timer_status()` → `number` (seconds remaining)
- `reset_timer()` → `void`
- `set_always_on_top(alwaysOnTop: boolean)` → `void`

## Events

- `timer-alert`: Emitted when 15-minute timer expires
- `start-recording`: Emitted when global shortcut is pressed

## Next Steps for Development

1. **Voice Recording Implementation**:
   - Integrate Whisper.cpp or use Web Audio API
   - Implement audio capture in frontend
   - Connect to transcription pipeline

2. **Icons**:
   - Create app icons in multiple sizes
   - Place in `src-tauri/icons/`

3. **Testing**:
   - Test Ollama integration with real transcripts
   - Test timer functionality
   - Test task CRUD operations

4. **Polish**:
   - Improve UI/UX
   - Add error handling
   - Add loading states
   - Add settings/preferences
