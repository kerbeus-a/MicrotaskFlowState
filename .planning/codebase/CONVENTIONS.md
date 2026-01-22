# Coding Conventions

**Analysis Date:** 2026-01-22

## Naming Patterns

**Files:**
- React components: PascalCase with `.tsx` extension (e.g., `TaskList.tsx`, `RecordButton.tsx`)
- Hooks: PascalCase with `.ts` extension, prefixed with `use` (e.g., `useAudioRecorder.ts`)
- Rust modules: snake_case with `.rs` extension (e.g., `commands.rs`, `database.rs`)
- CSS files: Match component name (e.g., `TaskList.css` paired with `TaskList.tsx`)

**Functions:**
- React components: PascalCase, exported as default (e.g., `export default function TaskList()`)
- React hooks: camelCase, prefixed with `use` and return interface (e.g., `export const useAudioRecorder = (): UseAudioRecorderReturn => {}`)
- Tauri commands: snake_case (e.g., `get_tasks`, `process_voice_recording`)
- Rust module functions: snake_case (e.g., `init_tables()`, `get_all_tasks()`)
- Helper functions: camelCase (e.g., `formatTime()`, `updateAudioLevel()`)

**Variables:**
- State variables: camelCase (e.g., `isRecording`, `timerRemaining`, `selectedDeviceId`)
- Constants: UPPER_SNAKE_CASE or camelCase depending on scope
- React props interfaces: PascalCase with `Props` suffix (e.g., `RecordButtonProps`, `TaskListProps`)
- Ref variables: camelCase with `Ref` suffix (e.g., `mediaRecorderRef`, `audioContextRef`, `streamRef`)

**Types:**
- Interfaces: PascalCase (e.g., `AudioDevice`, `Task`, `TaskResponse`, `UseAudioRecorderReturn`)
- Derived type exports with descriptive names (e.g., `AudioRecorderState`, `WhisperModelSize`)

## Code Style

**Formatting:**
- No explicit formatter configured (check for Prettier or ESLint config)
- TypeScript strict mode enabled in `tsconfig.json`
- Target: ES2020 with React JSX

**Linting:**
- TypeScript strict mode enabled with the following rules:
  - `noUnusedLocals: true` - Flags unused variables
  - `noUnusedParameters: true` - Flags unused parameters
  - `noFallthroughCasesInSwitch: true` - Prevents fall-through in switch cases
  - `strict: true` - Full strict type checking

## Import Organization

**Order:**
1. React and library imports (e.g., `import { useState } from "react"`)
2. External API/service imports (e.g., `import { invoke } from "@tauri-apps/api/core"`)
3. Local component imports (e.g., `import TaskList from "./components/TaskList"`)
4. Hook imports (e.g., `import { useAudioRecorder } from "./hooks/useAudioRecorder"`)
5. CSS/style imports (e.g., `import "./App.css"`)

**Example from `src/App.tsx`:**
```typescript
import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import TaskList from "./components/TaskList";
import RecordButton from "./components/RecordButton";
import TimerBar from "./components/TimerBar";
import SettingsModal from "./components/SettingsModal";
import AudioVisualizer from "./components/AudioVisualizer";
import PinIcon from "./components/PinIcon";
import { useAudioRecorder } from "./hooks/useAudioRecorder";
import "./App.css";
```

**Path Aliases:**
- Not configured; imports use relative paths

## Error Handling

**Patterns in TypeScript/React:**

1. **Try-catch with type narrowing:**
```typescript
try {
  await audioRecorder.startRecording();
} catch (error) {
  const errorMessage = error instanceof Error ? error.message : String(error);
  setProcessingError(errorMessage);
  setTimeout(() => setProcessingError(null), 5000);
}
```

2. **Error message improvement with context-specific patterns:**
```typescript
if (errorMessage.includes('Ollama') || errorMessage.includes('404')) {
  errorMessage = 'Ollama is not running. Please start Ollama (it should run on port 11434).';
} else if (errorMessage.includes('Unable to decode audio data')) {
  errorMessage = 'Unable to decode audio data. The recording may be empty or corrupted.';
}
```

3. **Async error handling with promise chaining:**
```typescript
await Promise.all([
  invoke<number>("get_timer_status"),
  invoke<number>("get_timer_duration"),
]).catch(() => {
  // Ignore sync errors - keep using local state
});
```

**Patterns in Rust:**

1. **Result type propagation with `.map_err()`:**
```rust
crate::database::get_all_tasks(&db)
    .map_err(|e: rusqlite::Error| e.to_string())
    .map(|tasks| tasks.into_iter().map(/* transform */).collect())
```

2. **Error conversion to String:**
```rust
pub fn get_tasks(db: State<Database>) -> Result<Vec<TaskResponse>, String> {
    crate::database::get_all_tasks(&db)
        .map_err(|e: rusqlite::Error| e.to_string())
}
```

3. **Option handling with `.ok()` or pattern matching:**
```rust
if let Ok(existing) = crate::database::find_and_complete_task(&db, &task.text) {
    // Handle success
} else {
    // Handle error
}
```

## Logging

**Framework:** `console.*` methods (browser) and `eprintln!()` / `println!()` (Rust)

**Patterns:**

1. **React console logging:**
```typescript
console.warn("⚠️ Running in browser mode. Tauri features will not work.");
console.error("Failed to load tasks:", error);
```

2. **Rust logging:**
```rust
eprintln!("Failed to setup global shortcut: {}", e);
eprintln!("Failed to register shortcut {}: {}. Trying next...", shortcut, e);
```

3. **Conditional logging in Rust:**
- Use error prints for critical issues
- Debug mode has additional console output for troubleshooting

## Comments

**When to Comment:**
- Complex audio processing logic (e.g., RMS calculation in `useAudioRecorder.ts` lines 213-225)
- Non-obvious platform-specific workarounds (e.g., Kaspersky microphone access issues)
- State management explanations (e.g., "Save window state on move/resize (debounced)")
- Disabled code with rationale (e.g., "Global shortcut setup - DISABLED" in `main.rs`)

**Examples:**
```typescript
// Check if track got muted (Kaspersky might mute it after recording starts)
const tracks = streamRef.current?.getAudioTracks() || [];

// WAV header
const writeString = (offset: number, string: string) => { ... };

// Use both RMS and peak for better detection
// RMS is better for average level, peak is better for detecting any sound
const rmsLevel = Math.min(100, Math.max(0, (rms * 1000)));
```

**JSDoc/TSDoc:**
- Minimal usage observed
- Function interfaces are documented through TypeScript interface definitions:
```typescript
export interface UseAudioRecorderReturn {
  startRecording: () => Promise<void>;
  stopRecording: () => Promise<Blob>;
  pauseRecording: () => void;
  resumeRecording: () => void;
  state: AudioRecorderState;
  error: string | null;
  availableDevices: AudioDevice[];
  selectedDeviceId: string | null;
  setSelectedDeviceId: (deviceId: string) => void;
  refreshDevices: () => Promise<void>;
}
```

## Function Design

**Size:**
- Functions generally keep logic focused and under 50 lines for readability
- Complex operations split into helper functions
- Example: `updateAudioLevel()` in `useAudioRecorder.ts` (lines 184-233) handles RMS calculation, peak detection, and state updates

**Parameters:**
- Prefer destructuring for React component props:
```typescript
export default function RecordButton({
  isRecording,
  isProcessing = false,
  recordingTime = 0,
  onStartRecording,
  onStopRecording,
}: RecordButtonProps)
```

- Use default parameter values for optional props
- Rust commands accept `State<T>` for managed state injection:
```rust
pub fn get_tasks(db: State<Database>) -> Result<Vec<TaskResponse>, String>
```

**Return Values:**
- React components: JSX element
- React hooks: Return interfaces that group related functionality:
```typescript
return {
  startRecording,
  stopRecording,
  pauseRecording,
  resumeRecording,
  state,
  error,
  availableDevices,
  selectedDeviceId,
  setSelectedDeviceId,
  refreshDevices,
};
```

- Tauri commands: `Result<T, String>` for consistent error handling
- Helper functions: Typed return values (e.g., `() => Promise<Blob>`, `() => void`)

## Module Design

**Exports:**
- Components: Default export of the component function
```typescript
export default function TaskList({ tasks, onToggle, onDelete, onUpdate }: TaskListProps)
```

- Hooks: Named export with `export const` pattern
```typescript
export const useAudioRecorder = (): UseAudioRecorderReturn => { ... }
```

- Interfaces and types: Named exports before function definitions
```typescript
export interface AudioDevice { ... }
export interface AudioRecorderState { ... }
export interface UseAudioRecorderReturn { ... }
export const useAudioRecorder = (): UseAudioRecorderReturn => { ... }
```

- Rust modules: Public functions with `pub fn` and public structs with `pub struct`

**Barrel Files:**
- Not used; components and hooks imported directly

---

*Convention analysis: 2026-01-22*
