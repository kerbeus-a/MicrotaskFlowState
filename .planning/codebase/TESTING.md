# Testing Patterns

**Analysis Date:** 2026-01-22

## Test Framework

**Status:** Not configured

**Investigation Results:**
- No `jest.config.js`, `vitest.config.ts`, or `.mocharc` files found
- No test files in `src/` directory (only `node_modules/` contains test files from dependencies)
- No test scripts in `package.json` (only `dev`, `build`, `preview`, `tauri`)
- No testing libraries listed in dependencies

**Run Commands:**
```bash
npm run dev              # Start dev server (includes Tauri)
npm run build            # Build for production
npm run tauri            # Run Tauri CLI
```

**Recommendation:** Testing infrastructure needs to be established.

## Test File Organization

**Location:** Not applicable (no tests exist)

**Proposed Pattern for Future Tests:**
- Co-locate tests with components: `src/components/TaskList.test.tsx` paired with `src/components/TaskList.tsx`
- Hook tests in same directory: `src/hooks/useAudioRecorder.test.ts` paired with `src/hooks/useAudioRecorder.ts`
- Backend tests in `src-tauri/tests/` directory with Rust testing conventions

**Naming:**
- React/TypeScript: `[Component].test.tsx` or `[Hook].test.ts`
- Rust: `#[cfg(test)]` modules within files or separate `tests/` directory files

**Directory Structure (Proposed):**
```
src/
├── components/
│   ├── TaskList.tsx
│   ├── TaskList.test.tsx          # Unit test
│   ├── RecordButton.tsx
│   ├── RecordButton.test.tsx
│   └── [other components]
├── hooks/
│   ├── useAudioRecorder.ts
│   └── useAudioRecorder.test.ts
└── App.test.tsx

src-tauri/
├── src/
│   ├── commands.rs
│   ├── database.rs
│   └── [other modules]
└── tests/
    ├── integration_tests.rs        # For end-to-end tests
```

## Test Structure

**Not Yet Implemented**

**Recommended Framework Choices:**

1. **For React/TypeScript Components:**
   - **Framework:** Vitest (modern, fast, ESM-native)
   - **Assertion:** Vitest built-in assertions or Chai
   - **DOM Testing:** React Testing Library for user-centric testing
   - **Installation:** `npm install -D vitest @testing-library/react @testing-library/user-event happy-dom`

2. **For Rust Backend:**
   - **Framework:** Built-in `#[cfg(test)]` and `#[test]` macros
   - **Assertions:** standard `assert!()`, `assert_eq!()` macros
   - **Integration Tests:** Separate files in `tests/` directory

**Proposed Setup Pattern for React Components:**

```typescript
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import TaskList from './TaskList';

describe('TaskList', () => {
  const mockTasks = [
    { id: 1, text: 'Task 1', completed: false, created_at: '2026-01-22', completed_at: null },
    { id: 2, text: 'Task 2', completed: true, created_at: '2026-01-21', completed_at: '2026-01-22' }
  ];

  it('renders active tasks separately from completed tasks', () => {
    const onToggle = vi.fn();
    const onDelete = vi.fn();
    const onUpdate = vi.fn();

    render(
      <TaskList
        tasks={mockTasks}
        onToggle={onToggle}
        onDelete={onDelete}
        onUpdate={onUpdate}
      />
    );

    expect(screen.getByText('Task 1')).toBeInTheDocument();
    expect(screen.getByText('Completed')).toBeInTheDocument();
  });

  it('calls onToggle when checkbox is clicked', async () => {
    const onToggle = vi.fn();
    const user = userEvent.setup();

    render(
      <TaskList
        tasks={[mockTasks[0]]}
        onToggle={onToggle}
        onDelete={() => {}}
        onUpdate={() => {}}
      />
    );

    const checkbox = screen.getByRole('checkbox');
    await user.click(checkbox);
    expect(onToggle).toHaveBeenCalledWith(1);
  });
});
```

**Proposed Setup Pattern for React Hooks:**

```typescript
import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useAudioRecorder } from './useAudioRecorder';

describe('useAudioRecorder', () => {
  beforeEach(() => {
    // Reset mocks between tests
    vi.clearAllMocks();
  });

  it('initializes with default state', () => {
    const { result } = renderHook(() => useAudioRecorder());

    expect(result.current.state.isRecording).toBe(false);
    expect(result.current.state.recordingTime).toBe(0);
    expect(result.current.error).toBeNull();
  });

  it('starts recording when startRecording is called', async () => {
    const { result } = renderHook(() => useAudioRecorder());

    await act(async () => {
      await result.current.startRecording();
    });

    expect(result.current.state.isRecording).toBe(true);
  });
});
```

## Mocking

**Framework (Proposed):** Vitest's built-in `vi` object (compatible with Jest API)

**Patterns for React Testing:**

```typescript
import { vi } from 'vitest';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

// Mock specific functions
const mockInvoke = vi.fn();
vi.mocked(invoke).mockImplementation(mockInvoke);

// Reset between tests
beforeEach(() => {
  vi.clearAllMocks();
});
```

**Patterns for React Hooks:**

```typescript
// Mock navigator.mediaDevices (audio recording)
global.navigator.mediaDevices = {
  getUserMedia: vi.fn(() => Promise.resolve({
    getTracks: () => [],
    getAudioTracks: () => [],
  } as any)),
  enumerateDevices: vi.fn(() => Promise.resolve([])),
  addEventListener: vi.fn(),
  removeEventListener: vi.fn(),
};

// Mock AudioContext
global.AudioContext = vi.fn(() => ({
  createMediaStreamSource: vi.fn(),
  createAnalyser: vi.fn(() => ({
    fftSize: 2048,
    smoothingTimeConstant: 0.3,
    getByteTimeDomainData: vi.fn(),
  })),
  createGain: vi.fn(),
  destination: {},
  state: 'running',
  resume: vi.fn(),
  close: vi.fn(),
  decodeAudioData: vi.fn(),
  currentTime: 0,
}));
```

**What to Mock:**
- External API calls (`invoke()` from Tauri)
- Browser APIs that may not be available in test environment (AudioContext, MediaRecorder, navigator.mediaDevices)
- Timer functions (`setTimeout`, `setInterval`) when testing debounced behavior

**What NOT to Mock:**
- React hooks (`useState`, `useEffect`, `useRef`, `useCallback`)
- Component props and callbacks (test the actual interaction)
- Local utility functions (test their actual behavior)
- Database logic (use fixtures or integration tests with real database)

## Fixtures and Factories

**Test Data (Proposed Pattern):**

```typescript
// test/fixtures/tasks.ts
export const createTask = (overrides?: Partial<Task>): Task => ({
  id: 1,
  text: 'Default task',
  completed: false,
  created_at: new Date().toISOString(),
  completed_at: null,
  ...overrides,
});

export const mockTasks = {
  active: [
    createTask({ id: 1, text: 'Active task 1' }),
    createTask({ id: 2, text: 'Active task 2' }),
  ],
  completed: [
    createTask({ id: 3, text: 'Completed task', completed: true, completed_at: new Date().toISOString() }),
  ],
  all: [
    createTask({ id: 1, text: 'Task 1' }),
    createTask({ id: 2, text: 'Task 2', completed: true }),
  ],
};

// test/fixtures/audio-devices.ts
export const mockAudioDevices = [
  { deviceId: 'default', label: 'Default Audio Input' },
  { deviceId: 'device-1', label: 'Microphone 1' },
  { deviceId: 'device-2', label: 'USB Microphone' },
];
```

**Location (Proposed):**
- `src/__tests__/fixtures/` for test fixtures
- `src/__tests__/mocks/` for mock implementations
- Or co-locate in same directory as tests

## Coverage

**Requirements:** Not configured (none enforced)

**Proposed Configuration (if testing is added):**

```json
// package.json (for vitest)
{
  "scripts": {
    "test": "vitest",
    "test:coverage": "vitest --coverage",
    "test:ui": "vitest --ui"
  },
  "devDependencies": {
    "@vitest/coverage-v8": "^latest"
  }
}
```

**Coverage Configuration (vitest.config.ts):**
```typescript
export default defineConfig({
  test: {
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        'src/**/*.test.{ts,tsx}',
        'src/**/__tests__/**',
        'node_modules/',
      ],
      lines: 80,
      functions: 80,
      branches: 80,
      statements: 80,
    },
  },
});
```

**View Coverage:**
```bash
npm run test:coverage      # Generate coverage report
npm run test:ui            # View coverage in browser
```

## Test Types

**Unit Tests (Proposed):**
- Scope: Individual components and hooks
- Approach: Test component rendering, prop changes, and callback invocations
- Example: `TaskList` rendering with different task states, `RecordButton` with recording/processing states
- Tools: React Testing Library + Vitest

**Integration Tests (Proposed):**
- Scope: Component interactions and state flow between parent/child components
- Approach: Test complete user flows (e.g., recording audio → processing → task creation)
- Example: Full App component with mocked Tauri API
- Tools: React Testing Library + Vitest with mocked Tauri

**E2E Tests (Proposed):**
- Framework: Playwright or Cypress (not yet configured)
- Scope: Full application flow in actual Tauri window
- Would test: Record button → audio capture → backend processing → task display
- Setup: `npm install -D @playwright/test` or `cypress`

**Backend Tests (Rust - Proposed):**
- Unit tests within modules using `#[cfg(test)]`
- Integration tests for database operations in `tests/` directory
- Example:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_task() {
        let db = Database::new().expect("Failed to create test DB");
        let task = add_task(&db, "Test task").unwrap();
        assert_eq!(task.text, "Test task");
        assert!(!task.completed);
    }
}
```

## Common Patterns

**Async Testing:**

```typescript
// Using async/await with waitFor
it('loads tasks on mount', async () => {
  const { result } = renderHook(() => useTasks());

  await waitFor(() => {
    expect(result.current.loading).toBe(false);
  });

  expect(result.current.tasks).toHaveLength(2);
});

// Using act for state updates
await act(async () => {
  await result.current.startRecording();
});
expect(result.current.state.isRecording).toBe(true);
```

**Error Testing:**

```typescript
// Testing error states
it('handles microphone permission denial', async () => {
  global.navigator.mediaDevices.getUserMedia = vi.fn(
    () => Promise.reject(new Error('NotAllowedError'))
  );

  const { result } = renderHook(() => useAudioRecorder());

  await act(async () => {
    try {
      await result.current.startRecording();
    } catch (e) {
      // Expected error
    }
  });

  expect(result.current.error).toContain('permission denied');
});

// Testing async error handling
it('catches invoke errors', async () => {
  mockInvoke.mockRejectedValue(new Error('Ollama is not running'));

  const { result } = renderHook(() => useApp());

  await act(async () => {
    await result.current.loadTasks();
  });

  expect(result.current.error).toContain('Ollama');
});
```

**Testing Debounced Behavior:**

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';

it('debounces timer duration changes', async () => {
  vi.useFakeTimers();
  const mockInvoke = vi.fn();

  // Component calls invoke after 500ms
  act(() => {
    handleTimerDurationChange(20);
    handleTimerDurationChange(25);
    handleTimerDurationChange(30);
  });

  // Invoke should only be called once, 500ms after last change
  vi.advanceTimersByTime(500);

  expect(mockInvoke).toHaveBeenCalledOnce();
  expect(mockInvoke).toHaveBeenCalledWith('set_timer_duration', { minutes: 30 });

  vi.useRealTimers();
});
```

**Testing Window/Event Listeners:**

```typescript
it('saves window state on resize', async () => {
  vi.useFakeTimers();
  const saveWindowState = vi.fn();

  const { rerender } = render(<App />);

  // Simulate window resize
  fireEvent(window, new Event('resize'));

  // Debounced save should happen after 500ms
  vi.advanceTimersByTime(500);

  expect(saveWindowState).toHaveBeenCalled();
});
```

---

*Testing analysis: 2026-01-22*
