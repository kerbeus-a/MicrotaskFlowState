# Codebase Structure

**Analysis Date:** 2026-01-22

## Directory Layout

```
MicroTask/
├── src/                              # React frontend (TypeScript)
│   ├── main.tsx                      # Entry point, mounts React app
│   ├── App.tsx                       # Root component, main UI logic
│   ├── App.css                       # Root component styles
│   ├── styles.css                    # Global styles
│   ├── components/                   # Reusable React components
│   │   ├── TaskList.tsx              # Task display and management
│   │   ├── TaskList.css
│   │   ├── RecordButton.tsx          # Voice recording button
│   │   ├── RecordButton.css
│   │   ├── TimerBar.tsx              # Timer display bar
│   │   ├── TimerBar.css
│   │   ├── SettingsModal.tsx         # Settings dialog for preferences
│   │   ├── SettingsModal.css
│   │   ├── AudioVisualizer.tsx       # Real-time audio level visualization
│   │   ├── AudioVisualizer.css
│   │   ├── ModelManager.tsx          # Whisper model download/management
│   │   ├── ModelManager.css
│   │   ├── PinIcon.tsx               # Always-on-top window toggle icon
│   │   └── PinIcon.css
│   └── hooks/                        # Custom React hooks
│       └── useAudioRecorder.ts       # Audio recording logic with device management
│
├── src-tauri/                        # Rust backend (Tauri)
│   ├── src/
│   │   ├── main.rs                   # Application entry, initialization, command registration
│   │   ├── commands.rs               # Tauri command handlers (API endpoints)
│   │   ├── database.rs               # SQLite operations, task CRUD, persistence
│   │   ├── timer.rs                  # Timer logic, alerts, state management
│   │   ├── whisper.rs                # Speech-to-text model management, transcription
│   │   ├── ollama.rs                 # Local LLM integration, task extraction from transcripts
│   │   └── native_main.rs            # Native UI entry point (unused, for future egui support)
│   ├── Cargo.toml                    # Rust dependencies and features
│   ├── build.rs                      # Build script for Tauri
│   ├── capabilities/
│   │   └── default.json              # Tauri security capabilities and permissions
│   └── target/                       # Build artifacts (gitignored)
│
├── index.html                        # HTML template
├── tsconfig.json                     # TypeScript compiler options for frontend
├── tsconfig.node.json                # TypeScript config for build tools
├── vite.config.ts                    # Vite build configuration
├── package.json                      # npm dependencies and scripts
├── package-lock.json                 # Dependency lock file
│
├── .planning/                        # GSD planning documents
│   └── codebase/
│       ├── ARCHITECTURE.md
│       ├── STRUCTURE.md
│       └── (other analysis docs)
│
└── dist/                             # Compiled output (gitignored)
```

## Directory Purposes

**src/** - React Frontend
- Purpose: User interface and interaction layer
- Contains: React components in TypeScript, custom hooks, CSS styling
- Key files: `App.tsx` (orchestrates all components), `main.tsx` (bootstrap)
- Output: Compiled to `dist/` by Vite

**src-tauri/** - Rust Backend and Desktop Integration
- Purpose: System-level operations, audio processing, data persistence
- Contains: Rust modules for database, timer, speech-to-text, LLM integration
- Key file: `main.rs` sets up Tauri, initializes services, registers commands
- Output: Compiled to native executable with embedded web view

**src/components/** - Reusable UI Components
- Purpose: Encapsulate UI logic and styling
- Pattern: Each component has .tsx and corresponding .css file
- State flow: Props from App.tsx, local state for UI control
- Examples:
  - `RecordButton.tsx` - Handles pointer events for press-and-hold recording
  - `TaskList.tsx` - Displays tasks with inline editing and completion toggle
  - `TimerBar.tsx` - Shows timer countdown and progress
  - `SettingsModal.tsx` - Modal for configuration (Whisper models, devices, Ollama toggle)

**src/hooks/** - Custom React Hooks
- Purpose: Extract and reuse stateful logic
- Current: `useAudioRecorder.ts` encapsulates Web Audio API, device enumeration, WAV encoding
- Pattern: Hooks return state and functions, manage refs for Web API objects

**src-tauri/src/** - Rust Backend Modules
- **main.rs**: Application bootstrap and Tauri setup
- **commands.rs**: IPC command handlers (implements public API)
  - Task operations: get_tasks, add_task, update_task, delete_task, toggle_task
  - Audio operations: process_voice_recording, transcribe_audio, save_audio_file
  - Timer operations: get_timer_status, reset_timer, set_timer_duration
  - Model operations: list_whisper_models, download_whisper_model, check_whisper_model
  - Settings: get/set always_on_top, window state, Ollama enabled, autostart

- **database.rs**: SQLite database layer
  - Manages connection with Mutex<Connection>
  - Initializes schema (tasks table, settings table)
  - CRUD operations for tasks
  - Settings persistence (Ollama enabled, timer duration)

- **timer.rs**: Awareness timer service
  - Background thread checks timer every 10 seconds
  - Emits "timer-alert" event when expired
  - Loads/saves duration configuration
  - Syncs state with frontend

- **whisper.rs**: Speech-to-text integration
  - Manages WhisperContext with model caching
  - Recovers from poisoned locks
  - Handles model downloads and existence checks
  - Transcribes audio buffers

- **ollama.rs**: Local LLM for task extraction
  - Parses transcripts into task actions
  - Extracts completion patterns ("done with X")
  - Extracts delete patterns ("delete X")
  - Extracts add patterns ("add task X")
  - Fallback to pattern-based parsing if Ollama unavailable

## Key File Locations

**Entry Points:**
- `src/main.tsx` - Frontend bootstrap, React render call
- `src-tauri/src/main.rs` - Backend bootstrap, Tauri builder setup
- `index.html` - HTML root document

**Configuration:**
- `vite.config.ts` - Frontend build configuration (fixed port 1420, Tauri setup)
- `tsconfig.json` - TypeScript compiler settings (strict mode enabled, ES2020 target)
- `package.json` - Frontend dependencies and npm scripts
- `src-tauri/Cargo.toml` - Backend dependencies and features

**Core Logic:**
- `src/App.tsx` - Central component, state management, Tauri API calls
- `src/hooks/useAudioRecorder.ts` - Audio recording state and Web Audio API
- `src-tauri/src/commands.rs` - API implementations, IPC handlers
- `src-tauri/src/database.rs` - Data access layer, SQLite operations
- `src-tauri/src/timer.rs` - Background timer service
- `src-tauri/src/whisper.rs` - Whisper model caching and transcription
- `src-tauri/src/ollama.rs` - Transcript parsing and task extraction

**Component UI:**
- `src/components/RecordButton.tsx` - Record button with press-and-hold logic
- `src/components/TaskList.tsx` - Task list with inline editing
- `src/components/TimerBar.tsx` - Timer display
- `src/components/SettingsModal.tsx` - Settings dialog
- `src/components/AudioVisualizer.tsx` - Real-time audio level bars
- `src/components/ModelManager.tsx` - Model download UI

## Naming Conventions

**Files:**
- React components: `PascalCase.tsx` for components, `.css` for styles (e.g., `RecordButton.tsx`, `RecordButton.css`)
- Hooks: `camelCase.ts` with `use` prefix (e.g., `useAudioRecorder.ts`)
- Rust modules: `snake_case.rs` (e.g., `commands.rs`, `whisper.rs`, `ollama.rs`)
- CSS files: Match component name exactly
- Configuration: `snake_case` or `camelCase` depending on type (e.g., `vite.config.ts`, `tsconfig.json`)

**Directories:**
- React folders: `camelCase` plural (e.g., `components/`, `hooks/`)
- Rust folders: `lowercase` (e.g., `src-tauri/`)
- Feature branches: Separate namespace (e.g., `.planning/`)

**TypeScript/JavaScript:**
- Interfaces: `PascalCase` with `I` prefix optional (e.g., `Task`, `TaskListProps`, `AudioRecorderState`)
- Functions: `camelCase` (e.g., `loadTasks`, `handleRecordClick`, `updateAudioLevel`)
- Constants: `UPPER_SNAKE_CASE` (e.g., `TIMER_START`)
- React event handlers: `handle[Event]` pattern (e.g., `handleStartRecording`, `handleDeleteTask`)

**Rust:**
- Structs: `PascalCase` (e.g., `Database`, `Task`, `WhisperCache`)
- Functions: `snake_case` (e.g., `init_database`, `get_tasks`, `process_voice_recording`)
- Types/Enums: `PascalCase` (e.g., `WhisperModelSize`, `TaskAction`)
- Static variables: `UPPER_SNAKE_CASE` (e.g., `TIMER_START`)

## Where to Add New Code

**New Feature (Voice Command):**
- Primary code: `src-tauri/src/commands.rs` - Add command handler
- Data layer: `src-tauri/src/database.rs` - Add database operations if needed
- Business logic: `src-tauri/src/ollama.rs` - Add transcript parsing patterns
- Frontend: `src/App.tsx` - Call new command via `invoke()`
- UI: `src/components/` - Add/modify component to trigger feature

**New React Component:**
- Implementation: `src/components/ComponentName.tsx` (TypeScript + JSX)
- Styling: `src/components/ComponentName.css` (paired CSS file)
- Props interface: Defined at top of .tsx file
- Export: Export default function from component file
- Import: Import in `src/App.tsx` or parent component

**New Custom Hook:**
- Implementation: `src/hooks/useFeatureName.ts`
- Return type: Interface named `UseFeatureNameReturn`
- Pattern: Return object with state properties and functions
- Example pattern: See `useAudioRecorder.ts` (lines 102-504)

**Utility Functions:**
- Shared utilities: `src/utils/` (create if needed) - But currently none exist, so keep in App.tsx or hooks
- Backend utilities: `src-tauri/src/` as new module file if large, otherwise inline in commands.rs

**Styling:**
- Global styles: `src/styles.css`
- Component styles: `src/components/ComponentName.css`
- CSS modules: Not used currently, prefer component-scoped CSS files
- Pattern: Import CSS in component file (e.g., `import "./RecordButton.css"`)

**Tests:**
- Not currently present in codebase
- If adding: Create `src/__tests__/` or `src/components/__tests__/` directories
- Or co-locate `.test.ts` files with source files
- Backend: Consider integration tests in `src-tauri/tests/`

## Special Directories

**node_modules/** - npm Dependencies
- Purpose: Frontend dependencies (React, Tauri API, build tools)
- Generated: Yes (from `npm install`)
- Committed: No (in .gitignore)
- Never edit manually

**src-tauri/target/** - Rust Build Artifacts
- Purpose: Compiled Rust code and dependencies
- Generated: Yes (from `cargo build`)
- Committed: No (in .gitignore)
- Never edit manually

**.planning/codebase/** - GSD Analysis Documents
- Purpose: Repository analysis for code generation
- Generated: Manual (by GSD commands)
- Committed: Yes
- Files: ARCHITECTURE.md, STRUCTURE.md, CONVENTIONS.md, TESTING.md, STACK.md, INTEGRATIONS.md, CONCERNS.md

**dist/** - Production Build Output
- Purpose: Compiled frontend bundle
- Generated: Yes (from `npm run build`)
- Committed: No (in .gitignore)
- Output of Vite build process

**.git/** - Version Control
- Purpose: Git repository metadata
- Generated: Automatic
- Committed: No (excluded)

---

*Structure analysis: 2026-01-22*
