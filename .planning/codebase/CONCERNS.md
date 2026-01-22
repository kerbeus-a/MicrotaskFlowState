# Codebase Concerns

**Analysis Date:** 2026-01-22

## Tech Debt

**Mutex Lock Unwrap Panics:**
- Issue: Extensive use of `.unwrap()` on `Mutex` locks throughout database and native_main, which will panic if the lock is poisoned (e.g., after a previous panic)
- Files: `src-tauri/src/database.rs` (lines 98, 124, 146, 155, 161, 196, 229, 246, 281, 291), `src-tauri/src/native_main.rs` (lines 180-185, 206, 316, 321, 358, 423, 709-715)
- Impact: A single panic in any database operation poisons the lock, causing cascading failures and app instability. Users must restart the app.
- Fix approach: Replace all `unwrap()` calls with `unwrap_or_else()` that recovers from poisoned locks (pattern already exists in `whisper.rs` lines 32-36)

**Expect Calls on File I/O:**
- Issue: Multiple `expect()` calls on filesystem operations that could legitimately fail
- Files: `src-tauri/src/database.rs` (lines 28, 82, 85)
- Impact: App panics if app data directory cannot be created (permissions issue, disk full, etc.)
- Fix approach: Propagate errors as `Result<>` instead of panicking, handle gracefully in calling code

**Global Timer State Using Static Mutex:**
- Issue: Timer state stored in static mutable globals (`TIMER_START`, `TIMER_DURATION` in `timer.rs` lines 5-6)
- Files: `src-tauri/src/timer.rs`
- Impact: Thread-unsafe initialization, potential race conditions, difficult to test, inflexible for future timer instances
- Fix approach: Move state into Tauri state management via `app.manage()`, similar to how Database and WhisperCache are managed

**Unwrap on Database Query Results:**
- Issue: `get_all_tasks(db)?.first().unwrap()` in `find_and_complete_task` (database.rs line 222)
- Files: `src-tauri/src/database.rs` (line 222)
- Impact: Panics if no tasks exist when trying to complete a task (empty task list scenario)
- Fix approach: Return proper error instead of unwrap, or return None and handle in caller

**Large Components Without Decomposition:**
- Issue: App.tsx (429 lines) contains all task management, timer, recording, and settings logic. useAudioRecorder.ts (504 lines) handles complex audio state with multiple refs and async callbacks.
- Files: `src/App.tsx`, `src/hooks/useAudioRecorder.ts`
- Impact: Difficult to test, hard to reason about, high cognitive load for maintenance
- Fix approach: Break App.tsx into smaller components (e.g., TaskSection, RecordingSection, HeaderSection), extract audio state machine logic

## Known Bugs

**Global Shortcut Disabled Without Clear Path Forward:**
- Issue: Global shortcut registration (Ctrl+Alt+R, F12, etc.) is completely disabled in main.rs (lines 26-118 are commented out) with no fallback
- Files: `src-tauri/src/main.rs` (lines 26-32, 78-118)
- Impact: Users cannot start recording from outside the app window; global shortcut feature is broken
- Workaround: None - must use app UI
- Fix approach: Implement robust retry logic with fallback shortcuts, or use a different global hotkey library

**AudioContext Created Multiple Times:**
- Issue: New `AudioContext` created every time recording stops (useAudioRecorder.ts line 408) and for chime playback (App.tsx line 278), which can leak resources
- Files: `src/hooks/useAudioRecorder.ts` (line 408), `src/App.tsx` (line 278)
- Impact: Resource leak, potential browser throttling of audio contexts, decreased performance over time
- Workaround: Restart app to clear resources
- Fix approach: Reuse single AudioContext instance, properly close it on cleanup

**Window State Persistence Race Condition:**
- Issue: Window save debounced to 500ms (App.tsx lines 93, 100) but app can close before debounce completes, losing window state
- Files: `src/App.tsx` (lines 89-101)
- Impact: Window position/size occasionally reverts to default on restart
- Fix approach: Use `beforeunload` event to flush pending saves before closing

**Kaspersky-Specific Audio Workarounds:**
- Issue: Code has multiple Kaspersky-specific hacks checking for muted state and trying to re-enable tracks (useAudioRecorder.ts lines 281-297, 341-352, 292-296)
- Files: `src/hooks/useAudioRecorder.ts`, `src/App.tsx` (lines 378-384)
- Impact: Adds complexity, may not work for other security software, hardcoded vendor name
- Fix approach: Generic permission denial detection, unified error messages that don't mention specific vendors

**Whisper Language Hardcoded to Russian:**
- Issue: `params.set_language(Some("ru"))` hardcoded in whisper.rs line 217 (Tauri build)
- Files: `src-tauri/src/whisper.rs` (line 217)
- Impact: Non-Russian users get degraded transcription quality without option to change
- Fix approach: Accept language parameter from frontend, store in database settings, expose in SettingsModal

**No Input Validation on Task Text:**
- Issue: Task text accepted without validation - no minimum length check, no XSS prevention, no trimming
- Files: `src-tauri/src/database.rs` (add_task, update_task)
- Impact: Empty tasks, leading/trailing whitespace tasks, potential injection vulnerabilities
- Fix approach: Add validation function: trim, minimum length 3, escape special characters

## Security Considerations

**Registry Access Without Validation:**
- Issue: Direct Windows registry manipulation in commands.rs lines 434-531 for autostart feature. No validation of exe_path before writing to registry.
- Files: `src-tauri/src/commands.rs` (lines 432-531)
- Current mitigation: Uses `std::env::current_exe()` which is relatively safe
- Recommendation: Add path validation to ensure exe_path is canonical and within expected locations. Consider using Windows autostart APIs instead of direct registry manipulation.

**Unencrypted Local Audio Storage:**
- Issue: Temporary audio files saved in plain WAV format in app data directory (commands.rs line 331)
- Files: `src-tauri/src/commands.rs` (line 330-332), `src/hooks/useAudioRecorder.ts` (line 404)
- Current mitigation: Files are in user's AppData directory (not world-readable on Windows)
- Recommendation: Delete temp files immediately after processing, avoid storing unencrypted audio at rest

**External Service Dependencies Without Retry Logic:**
- Issue: Ollama and Hugging Face model downloads have no retry logic, timeout handling is basic
- Files: `src-tauri/src/ollama.rs` (lines 344-351, 528-532), `src-tauri/src/whisper.rs` (lines 347-351)
- Current mitigation: Timeouts set (3s check, 15s transcription, 30s removal parse), fallback to simple parser
- Recommendation: Implement exponential backoff for download failures, validate response integrity with checksums

**No CORS Validation on Ollama Requests:**
- Issue: Ollama is accessed via HTTP (not HTTPS) with hardcoded localhost:11434
- Files: `src-tauri/src/ollama.rs` (lines 336-337, 430-431)
- Current mitigation: Localhost-only access reduces exposure
- Recommendation: Validate Ollama URL format, warn user if non-localhost Ollama is configured, document security implications

## Performance Bottlenecks

**Whisper Model Loading on Every Recording:**
- Issue: Before caching was added, model loaded on every voice recording (now fixed via WhisperCache, but native_main.rs doesn't use it)
- Files: `src-tauri/src/native_main.rs` (800 lines) - separate native build path doesn't have WhisperCache optimization
- Impact: 2-5s delay per recording in native build, poor UX
- Improvement path: Port WhisperCache to native_main.rs, share cache between both builds

**Audio Context Resume on Every Recording:**
- Issue: AudioContext.resume() called every time recording starts (useAudioRecorder.ts line 306), even if already resumed
- Files: `src/hooks/useAudioRecorder.ts` (line 305-306)
- Impact: Unnecessary async operations, potential timing issues
- Improvement path: Track audio context state, only resume if suspended

**Timer Polling Every 10 Seconds:**
- Issue: Backend timer checked every 10s via loop in timer.rs (lines 18-30), frontend also polls every 10s (App.tsx lines 72-74)
- Files: `src-tauri/src/timer.rs` (lines 17-30), `src/App.tsx` (lines 72-74)
- Impact: 10s drift possible between backend and frontend display, inaccurate for short durations
- Improvement path: Use event-driven alerts instead of polling, emit exact remaining time on expiry

**No Database Query Optimization:**
- Issue: `get_all_tasks` queries all tasks where completed=0 OR completed in last 7 days, no pagination, no limit
- Files: `src-tauri/src/database.rs` (lines 97-121)
- Impact: With thousands of tasks, query becomes slow, UI blocks during fetch
- Improvement path: Add pagination (LIMIT/OFFSET), index on completed_at, implement incremental sync

**Synchronous Mutex Lock in Tauri Commands:**
- Issue: Database operations lock entire connection during command execution (database.rs lines 98, 124, etc.)
- Files: `src-tauri/src/database.rs` (all database operation functions)
- Impact: Multiple recording operations or task updates can block each other
- Improvement path: Use connection pooling (e.g., r2d2), or async rusqlite wrapper

## Fragile Areas

**Ollama Integration with Two Parsing Paths:**
- Issue: Two completely different parsers: simple keyword-based in ollama.rs (lines 74-163) and Ollama LLM-based (lines 404-600), with fallback logic that silently switches between them
- Files: `src-tauri/src/ollama.rs` (lines 74-600), `src-tauri/src/commands.rs` (lines 375-390)
- Why fragile: Different behavior between modes, hard to debug which path was taken, transcription quality unpredictable
- Safe modification: Log which parser was used, add telemetry, document decision logic clearly
- Test coverage: No unit tests for either parser, manual testing only

**Audio Recording State Machine:**
- Issue: Recording state spread across 15+ refs (mediaRecorderRef, audioContextRef, analyserRef, streamRef, chunksRef, animationFrameRef, startTimeRef, timerIntervalRef, isRecordingRef, isPausedRef, deviceChangeTimeoutRef, selectedDeviceIdRef) in useAudioRecorder.ts
- Files: `src/hooks/useAudioRecorder.ts` (lines 113-124)
- Why fragile: Easy to forget to reset a ref, state can get out of sync, difficult to reason about invariants
- Safe modification: Refactor into formal state machine with explicit state transitions
- Test coverage: No unit tests for state transitions

**Task Matching by Fuzzy Text:**
- Issue: Task completion/deletion uses LIKE '%text%' (database.rs lines 199-204, 249-258) with no exact match priority until SQL fix
- Files: `src-tauri/src/database.rs` (lines 199-225, 245-278)
- Why fragile: "edit" voice command might complete "edited" or "editor" instead of intended task, leading to wrong task marked done
- Safe modification: Improve matching algorithm (Levenshtein distance, exact-first), add confidence scoring, require user confirmation for ambiguous matches
- Test coverage: No tests for matching algorithm

**Whisper Language Detection:**
- Issue: Language hardcoded per build (Russian for Tauri, English for native) with no runtime configuration
- Files: `src-tauri/src/whisper.rs` (lines 217, 300)
- Why fragile: Multi-language input fails silently, transcription quality degrades without user visibility
- Safe modification: Detect language from input, allow user override in settings
- Test coverage: No language detection tests

**Window State Restore with Validation:**
- Issue: Window state restored with basic range checks (-1000 to positive) but no screen boundary validation (App.tsx lines 54-58)
- Files: `src/App.tsx` (lines 54-59)
- Why fragile: Window can restore off-screen if monitors were unplugged, user cannot interact with window
- Safe modification: Validate window is within any available monitor bounds before restoring
- Test coverage: No monitor boundary validation tests

## Scaling Limits

**SQLite Concurrency Bottleneck:**
- Current capacity: Single connection with Mutex, ~1 concurrent operation
- Limit: 2-3 simultaneous recording operations cause lock contention
- Scaling path: Migrate to connection pool (r2d2 or sqlx), or use async database (better-sqlite3 replacement)

**Whisper Model Cache - Single Model:**
- Current capacity: One model loaded in memory at a time (~2.9GB max for Large model)
- Limit: Cannot switch between models without 5-10s reload time
- Scaling path: Pre-load multiple models, or lazy-load with size-aware eviction policy

**Ollama Response Timeout - Fixed 15 Seconds:**
- Current capacity: Ollama response expected within 15s (ollama.rs line 529)
- Limit: Larger models or high-latency systems timeout frequently
- Scaling path: Make timeout configurable, implement adaptive timeout based on model size

## Dependencies at Risk

**whisper-rs 0.11 - LLVM Dependency:**
- Risk: Requires LLVM/Clang to compile, binary size bloat, platform-specific build issues
- Impact: Windows build fails on systems without LLVM, build time increases significantly
- Migration plan: Monitor whisper.cpp for pure-Rust implementations, consider pre-compiled binaries, or switch to OpenAI Whisper API

**tokio "full" Feature:**
- Risk: Pulling in all tokio features (io-uring, metrics, etc.) that app doesn't use, increases binary size and dependency complexity
- Impact: Slower compile times, larger app bundle
- Migration plan: Explicitly list required features (rt, sync, time, macros), remove "full"

**Windows 0.52 - Extensive Win32 Binding:**
- Risk: Windows crate is large, pulls deep Win32 API dependencies, tight coupling to Windows versions
- Impact: Brittle on Windows version changes, large binary size
- Migration plan: Use safer abstractions (winapi-core), or pure-Rust libraries where possible

**Cargo.toml Missing LLVM Requirement Documentation:**
- Risk: No explicit documentation of LLVM requirement or build environment setup
- Impact: Developers unable to build without discovering requirement through trial/error
- Migration plan: Add build.rs check with informative error, document in README

## Missing Critical Features

**No Offline Fallback:**
- Problem: App requires working network for initial Whisper model download. No bundled models, no offline-first UX.
- Blocks: Users cannot start recording until models downloaded (50MB-2.9GB depending on size)

**No Undo/Redo for Tasks:**
- Problem: Deleted or completed tasks cannot be recovered, no task history
- Blocks: Users who accidentally complete/delete important tasks have no recovery

**No Keyboard Shortcuts in UI:**
- Problem: No way to trigger recording, complete tasks, or navigate without mouse
- Blocks: Power users, accessibility concerns

**No Task Categories/Tags:**
- Problem: All tasks in flat list, no organization by project or priority
- Blocks: Users with 50+ tasks have no way to organize them

**No Sync Between Devices:**
- Problem: Tasks are local-only, cannot access from phone or other computer
- Blocks: Mobile task logging, team collaboration features

**No Task Search:**
- Problem: No way to find tasks in large list
- Blocks: Users cannot search for completed tasks or find specific tasks among many

## Test Coverage Gaps

**No Unit Tests for Task Parsing:**
- What's not tested: `parse_transcript_to_actions()` function with various inputs (Russian/English, action keywords, edge cases)
- Files: `src-tauri/src/ollama.rs` (lines 75-163)
- Risk: Regression in transcription-to-task conversion breaks silently, wrong actions parsed
- Priority: High

**No Integration Tests for Voice Recording Pipeline:**
- What's not tested: Full flow from audio capture -> transcription -> task creation, including error cases (network failure, model missing, bad audio)
- Files: `src-tauri/src/commands.rs` (lines 344-427), `src/hooks/useAudioRecorder.ts`
- Risk: Pipeline failures discovered only in production
- Priority: High

**No Tests for Database Queries:**
- What's not tested: `find_and_complete_task`, `find_and_delete_task`, fuzzy matching logic
- Files: `src-tauri/src/database.rs` (lines 195-278)
- Risk: Silent matching failures cause wrong tasks to be modified
- Priority: High

**No Tests for Timer State Management:**
- What's not tested: Timer reset, duration changes, expired alert triggering, race conditions with concurrent operations
- Files: `src-tauri/src/timer.rs` (all)
- Risk: Timer skips, alerts never fire, or fire at wrong times
- Priority: Medium

**No Snapshot Tests for Audio Encoding:**
- What's not tested: WAV encoding correctness, downsampling accuracy, sample format conversions
- Files: `src/hooks/useAudioRecorder.ts` (lines 30-100), `src-tauri/src/whisper.rs` (lines 244-271)
- Risk: Audio corruption, transcription fails silently on invalid WAV
- Priority: Medium

**No Tests for Whisper Cache Behavior:**
- What's not tested: Model switching, poisoned lock recovery, concurrent access
- Files: `src-tauri/src/whisper.rs` (lines 17-80)
- Risk: Model cache bugs cause panics or stale results
- Priority: High

**No Browser Compatibility Tests:**
- What's not tested: App in browser mode (non-Tauri), browser console errors, API fallback behavior
- Files: `src/App.tsx` (lines 310-335)
- Risk: Browser mode error messages confusing, breaking changes in browser support undetected
- Priority: Low

**No Stress Tests:**
- What's not tested: 1000+ tasks in list, rapid recording (5+ simultaneous), long app uptime (memory leaks)
- Impact: Unknown scaling behavior, potential memory leaks undetected
- Priority: Medium

---

*Concerns audit: 2026-01-22*
