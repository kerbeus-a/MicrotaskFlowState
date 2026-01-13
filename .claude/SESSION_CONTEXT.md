# MicroTask FlowState - Session Context

## Project Overview
MicroTask FlowState is a Tauri v2 desktop app for voice-based task management. Users speak tasks into a microphone, the app transcribes using local Whisper models, and parses the transcript to create/complete/delete tasks.

## What Was Done This Session

### 1. Fixed App Hanging Issue - Whisper Model Caching
**Problem:** App hung for 30+ seconds on every voice recording because Whisper model (147MB) was reloading each time.

**Solution:** Implemented `WhisperCache` in `src-tauri/src/whisper.rs`:
- Model loads once at first recording, stays in memory
- Subsequent recordings use cached model (instant)
- Cache clears when user deletes a model
- Handles "poisoned lock" errors gracefully (recovers from panics)

**Files changed:**
- `src-tauri/src/whisper.rs` - Added `WhisperCache` struct
- `src-tauri/src/main.rs` - Initialize cache as Tauri managed state
- `src-tauri/src/commands.rs` - Use `whisper_cache.get_or_create()` instead of `WhisperEngine::new()`

### 2. Fixed Ollama Causing Long Delays
**Problem:** Ollama LLM parsing took 30+ minutes sometimes, blocking the UI.

**Solution:**
- Disabled Ollama by default (`USE_OLLAMA=false`)
- Using fast local simple parser instead
- Set `USE_OLLAMA=true` environment variable to enable Ollama if needed
- Reduced timeouts: 3s for connectivity check, 15s for generation

**File:** `src-tauri/src/ollama.rs`

### 3. Fixed Multiple Tasks Not Being Created
**Problem:** Saying "task1, task2, task3, task4" only created 1-2 tasks.

**Solution:** Updated simple parser to split on:
- Commas (`,`)
- Periods (`.`)
- Semicolons (`;`)
- " and " (English)
- " и " (Russian)

Each part becomes a separate task.

**File:** `src-tauri/src/ollama.rs` - `parse_transcript_to_actions()` function

### 4. Fixed Tauri v2 Permission Errors
**Problem:** `event.listen not allowed` error in Tauri v2.

**Solution:** Created `src-tauri/capabilities/default.json` with required permissions.

### 5. Added Whisper Hallucination Filtering
**Problem:** Whisper outputs "[музыка]", "Thank you", etc. becoming tasks.

**Solution:** `is_noise_transcript()` function filters out common hallucinations.

## Current State
- App works with instant voice-to-task conversion
- Whisper model caches in memory after first load
- Simple parser handles comma-separated tasks in Russian/English
- Ollama disabled by default (can enable with env var)

## Key Files
- `src-tauri/src/whisper.rs` - Whisper model loading, caching, transcription
- `src-tauri/src/ollama.rs` - Task parsing (simple parser + optional Ollama)
- `src-tauri/src/commands.rs` - Tauri commands for voice recording
- `src-tauri/src/main.rs` - App initialization, state management
- `src-tauri/capabilities/default.json` - Tauri v2 permissions

## How to Run
```bash
npm run tauri dev
```
Or release build:
```
E:\Dev_projects\MicroTask\src-tauri\target\release\flowstate.exe
```

## Environment Variables
- `USE_OLLAMA=true` - Enable Ollama for smarter parsing (disabled by default)
- `OLLAMA_URL` - Ollama server URL (default: http://localhost:11434)
- `OLLAMA_MODEL` - Ollama model name (default: llama3.2)

## Known Issues / TODOs
1. Close app from system tray before rebuilding (hot reload fails otherwise)
2. Whisper language is hardcoded to Russian (`set_language(Some("ru"))`)
3. Some unused functions generate compiler warnings (can be cleaned up)

## Test Phrase (Russian)
"Выпить воды, поесть мороженого, помыть кружку, почистить линзы"
Should create 4 tasks.
