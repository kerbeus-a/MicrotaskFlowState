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

TBD - Investigate:
1. Check if transcribed text is being passed to task creation logic
2. Verify simple parser is extracting tasks correctly
3. Check if tasks are being sent to frontend/stored in database
4. For CMD window: add `#![windows_subsystem = "windows"]` attribute or equivalent to hide console
