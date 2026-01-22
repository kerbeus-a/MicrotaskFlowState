---
created: 2026-01-22T10:58
title: Native app voice recording not parsed into tasks
area: audio
files:
  - src-tauri/src/whisper.rs
  - src-tauri/src/native_main.rs
  - src-tauri/src/main.rs
---

## Problem

When using the native app (flowstate-native.exe), voice recording is transcribed by Whisper (confirmed by "Transcribed:" and "Using simple parser (fast mode)" in console output), but the transcribed text does not appear in the task list.

Additionally, the native app launches with a visible CMD console window, which should be hidden for a clean user experience.

Screenshot shows:
- Whisper model loading successfully (ggml-base.bin)
- "Transcribed:" output appears (transcription working)
- "Using simple parser (fast mode)" indicates parsing attempted
- But no tasks appear in the UI task list

## Solution

**FIXED** in this session:

1. **CMD Console Hidden**: Added `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` to native_main.rs:1

2. **Sample Rate Bug Fixed**: The code captured `sample_rate` from the audio device but used hardcoded 48kHz for resampling. Now stores actual sample rate in `input_sample_rate` field and uses it for proper resampling to 16kHz (Whisper requirement).

3. **Better Error Handling**: Added checks for empty transcription with user-friendly error message, and debug logging throughout the pipeline.

**Files changed:**
- `src-tauri/src/native_main.rs` - All fixes applied
