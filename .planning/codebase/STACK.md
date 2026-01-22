# Technology Stack

**Analysis Date:** 2026-01-22

## Languages

**Primary:**
- TypeScript 5.2.2 - React frontend components and type definitions
- Rust 2021 edition - Backend services, database operations, audio processing, and desktop integration

**Secondary:**
- HTML/CSS - UI structure and styling (embedded in React components)

## Runtime

**Environment:**
- Node.js (version not explicitly specified, check `.nvmrc` or package scripts)
- Rust toolchain (1.56+)

**Package Manager:**
- npm (lockfile present: `package-lock.json`)
- Cargo (Rust package manager)

## Frameworks

**Core:**
- React 18.2.0 - Frontend UI library
- Tauri 2.0.0 - Desktop application framework (native Windows integration)

**React Plugins/Extensions:**
- @tauri-apps/api 2.0.0 - Tauri JavaScript API bridge
- @tauri-apps/plugin-dialog 2.0.0 - File/folder dialogs
- @tauri-apps/plugin-shell 2.0.0 - Shell command execution

**Build/Dev:**
- Vite 5.0.0 - Frontend bundler and dev server (configured at `vite.config.ts`, port 1420)
- @vitejs/plugin-react 4.2.0 - React Fast Refresh support
- TypeScript 5.2.2 - Type checking and transpilation (configured at `tsconfig.json`)

**Desktop/Native:**
- tauri-plugin-shell 2.0.0 - Execute shell commands from Tauri
- tauri-plugin-dialog 2.0.0 - Native file dialogs
- tauri-plugin-global-shortcut 2.0.0 - Global keyboard shortcuts (currently disabled in code)

## Key Dependencies

**Critical:**
- whisper-rs 0.11 - Local speech-to-text via OpenAI Whisper (requires LLVM/Clang)
- hound 3.5 - WAV audio file reading/writing
- reqwest 0.11 - HTTP client with JSON/streaming support

**Database:**
- rusqlite 0.30 - SQLite database binding with bundled SQLite

**Serialization:**
- serde 1.0 - Serialization framework
- serde_json 1.0 - JSON serialization

**Runtime/Async:**
- tokio 1.x (full features) - Async runtime
- futures-util 0.3 - Futures utilities

**Utilities:**
- chrono 0.4 - Date/time handling
- dirs 5.0 - Platform-specific directory paths (app data directories)
- windows 0.52 - Windows API bindings (Threading, UI, Registry)

**UI Framework Options (Feature-gated):**
- eframe 0.29 - egui framework wrapper (optional native UI alternative)
- cpal 0.15 - Cross-platform audio API (optional, for native UI)

## Configuration

**Environment:**
- `OLLAMA_URL` - Ollama server endpoint (defaults to `http://localhost:11434`)
- `OLLAMA_MODEL` - Ollama model name (defaults to `llama3.2`)

**Tauri:**
- Configuration via `src-tauri/tauri.conf.json` (Tauri 2.0)
- Feature flags in `Cargo.toml`: `tauri-ui` (default, webview) and `native-ui` (egui alternative)

**Build:**
- Vite config: `vite.config.ts` - React plugin, port 1420, watches `src/` but ignores `src-tauri/`
- TypeScript config: `tsconfig.json` - ES2020 target, strict mode enabled
- Node config: `tsconfig.node.json` - Referenced for bundler settings

## Platform Requirements

**Development:**
- Node.js and npm
- Rust toolchain (rustc, cargo)
- LLVM/Clang (required by whisper-rs)
- Windows SDK (for Windows.rs crate features)

**Production:**
- Windows desktop only (Tauri v2, uses webview on Windows)
- Local Ollama installation (optional, for enhanced LLM parsing)

## Binary Targets

**Tauri UI (default):**
- Binary: `flowstate`
- Main: `src-tauri/src/main.rs`
- Features: `tauri-ui`

**Native UI (alternative):**
- Binary: `flowstate-native`
- Main: `src-tauri/src/native_main.rs` (not yet implemented)
- Features: `native-ui` (eframe + cpal)

## Build Process

**Frontend:**
- TypeScript compilation via `tsc`
- Bundling via `vite build`
- Development server via `vite` (port 1420)

**Backend:**
- Compiled by `tauri-build` during Tauri build process
- Produces native Windows executable with embedded webview

---

*Stack analysis: 2026-01-22*
