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

// Check if running in Tauri (v2 uses __TAURI_INTERNALS__)
const isTauri = typeof window !== "undefined" && ("__TAURI_INTERNALS__" in window || "__TAURI_IPC__" in window);

interface Task {
  id: number;
  text: string;
  completed: boolean;
  created_at: string;
  completed_at: string | null;
}

function App() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);
  const [timerRemaining, setTimerRemaining] = useState(900); // 15 minutes in seconds
  const [timerDuration, setTimerDuration] = useState(15); // minutes
  const [alwaysOnTop, setAlwaysOnTop] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [hasWhisperModel, setHasWhisperModel] = useState<boolean | null>(null);
  const [selectedModel, setSelectedModel] = useState<string>("tiny");
  const [processingError, setProcessingError] = useState<string | null>(null);

  // Audio recorder hook
  const audioRecorder = useAudioRecorder();

  // Window state persistence
  const windowStateSaveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!isTauri) {
      console.warn("⚠️ Running in browser mode. Tauri features will not work. Please run 'npm run tauri dev' to launch the Tauri app.");
      return;
    }

    // Restore window state on startup (after a short delay to ensure window is ready)
    const restoreWindowState = async () => {
      try {
        // Wait a bit for window to be fully initialized
        await new Promise(resolve => setTimeout(resolve, 100));
        const savedState = await invoke<{ x: number; y: number; width: number; height: number } | null>("load_window_state");
        if (savedState) {
          // Validate state (ensure window fits on screen)
          if (savedState.width > 0 && savedState.height > 0 && savedState.x >= -1000 && savedState.y >= -1000) {
            await invoke("set_window_state", { state: savedState });
          }
        }
      } catch (error) {
        console.error("Failed to restore window state:", error);
      }
    };

    restoreWindowState();

    loadTasks();
    syncTimer(); // Initial sync only
    checkWhisperModels();

    // Local countdown - update every 10 seconds to minimize CPU usage
    const interval = setInterval(() => {
      setTimerRemaining(prev => (prev > 0 ? prev - 10 : prev));
    }, 10000);

    // Save window state on move/resize (debounced)
    const saveWindowState = async () => {
      try {
        const state = await invoke<{ x: number; y: number; width: number; height: number }>("get_window_state");
        await invoke("save_window_state", { state });
      } catch (error) {
        console.error("Failed to save window state:", error);
      }
    };

    const window = getCurrentWindow();
    
    // Listen for window move/resize events
    const unlistenMove = window.onMoved(() => {
      if (windowStateSaveTimeoutRef.current) {
        clearTimeout(windowStateSaveTimeoutRef.current);
      }
      windowStateSaveTimeoutRef.current = setTimeout(saveWindowState, 500);
    });

    const unlistenResize = window.onResized(() => {
      if (windowStateSaveTimeoutRef.current) {
        clearTimeout(windowStateSaveTimeoutRef.current);
      }
      windowStateSaveTimeoutRef.current = setTimeout(saveWindowState, 500);
    });

    // Listen for timer alerts
    const unlisten = listen("timer-alert", async () => {
      playChime();
      // Visual pulse effect
      document.body.style.animation = "pulse 0.5s";
      setTimeout(() => {
        document.body.style.animation = "";
      }, 500);
      // Sync timer after alert (backend resets it)
      try {
        const [remaining, duration] = await Promise.all([
          invoke<number>("get_timer_status"),
          invoke<number>("get_timer_duration"),
        ]);
        setTimerRemaining(remaining);
        setTimerDuration(duration);
      } catch {
        // Ignore sync errors
      }
    });

    // Listen for start-recording event (from global shortcut)
    const unlistenRecording = listen("start-recording", () => {
      handleRecordClick();
    });

    return () => {
      clearInterval(interval);
      if (windowStateSaveTimeoutRef.current) {
        clearTimeout(windowStateSaveTimeoutRef.current);
      }
      unlisten.then(fn => fn());
      unlistenRecording.then(fn => fn());
      unlistenMove.then(fn => fn());
      unlistenResize.then(fn => fn());
    };
  }, []);

  const checkWhisperModels = async () => {
    if (!isTauri) return;
    try {
      const models = await invoke<Array<{ name: string; installed: boolean }>>("list_whisper_models");
      const installedModels = models.filter(m => m.installed);
      const hasModel = installedModels.length > 0;
      setHasWhisperModel(hasModel);

      // Auto-select the first installed model
      if (hasModel) {
        const firstInstalled = installedModels[0];
        setSelectedModel(firstInstalled.name.toLowerCase());
      } else {
        // Show settings if no model is installed
        setTimeout(() => setShowSettings(true), 1000);
      }
    } catch (error) {
      console.error("Failed to check Whisper models:", error);
    }
  };

  const loadTasks = async () => {
    if (!isTauri) return;
    try {
      const loadedTasks = await invoke<Task[]>("get_tasks");
      setTasks(loadedTasks);
    } catch (error) {
      console.error("Failed to load tasks:", error);
    }
  };

  // Sync timer with backend (only called on mount and after reset)
  const syncTimer = async () => {
    if (!isTauri) return;
    try {
      const [remaining, duration] = await Promise.all([
        invoke<number>("get_timer_status"),
        invoke<number>("get_timer_duration"),
      ]);
      setTimerRemaining(remaining);
      setTimerDuration(duration);
    } catch (error) {
      console.error("Failed to get timer status:", error);
    }
  };

  const handleStartRecording = async () => {
    try {
      setProcessingError(null);
      await audioRecorder.startRecording();
    } catch (error) {
      console.error("Failed to start recording:", error);
      const errorMessage = error instanceof Error ? error.message : String(error);
      setProcessingError(errorMessage);
      setTimeout(() => setProcessingError(null), 5000);
    }
  };

  const handleStopRecording = async () => {
    // Stop recording and process
    try {
      setIsProcessing(true);
      setProcessingError(null);

      // Stop recording and get WAV blob
      const audioBlob = await audioRecorder.stopRecording();

      // Convert blob to array buffer
      const arrayBuffer = await audioBlob.arrayBuffer();
      const audioData = Array.from(new Uint8Array(arrayBuffer));

      // Process with backend
      await invoke("process_voice_recording", {
        audioData,
        modelName: selectedModel,
      });

      // Reload tasks to get the latest
      await loadTasks();
    } catch (error) {
      console.error("Failed to process voice recording:", error);
      let errorMessage = error instanceof Error ? error.message : String(error);
      
      // Improve error messages
      if (errorMessage.includes('Ollama') || errorMessage.includes('404')) {
        errorMessage = 'Ollama is not running. Please start Ollama (it should run on port 11434). The audio was recorded but cannot be transcribed without Ollama.';
      } else if (errorMessage.includes('Unable to decode audio data')) {
        errorMessage = 'Unable to decode audio data. The recording may be empty or corrupted. Check if the microphone is working and try again.';
      }
      
      setProcessingError(errorMessage);

      // Show error notification
      setTimeout(() => setProcessingError(null), 5000);
    } finally {
      setIsProcessing(false);
    }
  };

  // Legacy click handler for global shortcut compatibility
  const handleRecordClick = async () => {
    if (audioRecorder.state.isRecording) {
      await handleStopRecording();
    } else {
      await handleStartRecording();
    }
  };

  const handleToggleTask = async (id: number) => {
    try {
      await invoke("toggle_task", { id });
      await loadTasks();
    } catch (error) {
      console.error("Failed to toggle task:", error);
    }
  };

  const handleDeleteTask = async (id: number) => {
    try {
      await invoke("delete_task", { id });
      await loadTasks();
    } catch (error) {
      console.error("Failed to delete task:", error);
    }
  };

  const handleUpdateTask = async (id: number, text: string) => {
    try {
      await invoke("update_task", { id, text });
      await loadTasks();
    } catch (error) {
      console.error("Failed to update task:", error);
    }
  };

  const playChime = () => {
    // Create a simple chime sound
    const audioContext = new AudioContext();
    const oscillator = audioContext.createOscillator();
    const gainNode = audioContext.createGain();
    
    oscillator.connect(gainNode);
    gainNode.connect(audioContext.destination);
    
    oscillator.frequency.value = 800;
    oscillator.type = "sine";
    
    gainNode.gain.setValueAtTime(0.3, audioContext.currentTime);
    gainNode.gain.exponentialRampToValueAtTime(0.01, audioContext.currentTime + 0.5);
    
    oscillator.start(audioContext.currentTime);
    oscillator.stop(audioContext.currentTime + 0.5);
  };

  const toggleAlwaysOnTop = async () => {
    if (!isTauri) {
      alert("This feature only works in the Tauri app. Please run 'npm run tauri dev' to launch the app.");
      return;
    }
    try {
      const newValue = !alwaysOnTop;
      await invoke("set_always_on_top", { alwaysOnTop: newValue });
      setAlwaysOnTop(newValue);
    } catch (error) {
      console.error("Failed to toggle always on top:", error);
      alert(`Failed to toggle always on top: ${error}`);
    }
  };

  if (!isTauri) {
    return (
      <div className="app">
        <div className="app-content" style={{ padding: "40px", textAlign: "center" }}>
          <h1>FlowState</h1>
          <div style={{ marginTop: "40px", padding: "20px", background: "#2a2a2a", borderRadius: "8px" }}>
            <h2>⚠️ Browser Mode Detected</h2>
            <p style={{ marginTop: "20px", lineHeight: "1.6" }}>
              You're viewing this app in a browser. FlowState requires the Tauri desktop app to function.
            </p>
            <p style={{ marginTop: "20px", fontWeight: "bold" }}>
              To run the app properly:
            </p>
            <ol style={{ textAlign: "left", display: "inline-block", marginTop: "20px" }}>
              <li>Close this browser window</li>
              <li>Run: <code style={{ background: "#1a1a1a", padding: "4px 8px", borderRadius: "4px" }}>npm run tauri dev</code></li>
              <li>The Tauri app window will open automatically</li>
            </ol>
            <p style={{ marginTop: "30px", fontSize: "14px", color: "#888" }}>
              Tauri APIs (database, timer, models, etc.) only work in the Tauri window, not in a browser.
            </p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="app">
      <TimerBar remaining={timerRemaining} duration={timerDuration} />
      <div className="app-content">
        <div className="header">
          <h1>FlowState</h1>
          <div className="header-actions">
            <button
              className="settings-button"
              onClick={() => setShowSettings(true)}
              title="Settings"
            >
              ⚙️
            </button>
            <button
              className="always-on-top-toggle"
              onClick={toggleAlwaysOnTop}
              title={alwaysOnTop ? "Disable always on top" : "Enable always on top"}
            >
              <PinIcon isPinned={alwaysOnTop} />
            </button>
          </div>
        </div>
        <RecordButton
          isRecording={audioRecorder.state.isRecording}
          isProcessing={isProcessing}
          recordingTime={audioRecorder.state.recordingTime}
          onStartRecording={handleStartRecording}
          onStopRecording={handleStopRecording}
        />

        {/* Audio Visualizer - only show when recording */}
        {audioRecorder.state.isRecording && (
          <>
            <AudioVisualizer
              audioLevel={audioRecorder.state.audioLevel}
              isRecording={audioRecorder.state.isRecording}
            />
            {audioRecorder.state.audioLevel === 0 && (
              <div className="no-audio-warning">
                <p>⚠️ No audio detected. Check:</p>
                <ul>
                  <li>Microphone is not muted in Windows</li>
                  <li>Kaspersky is allowing microphone access</li>
                  <li>Microphone is selected correctly</li>
                  <li>Try the "Test Mic" button above</li>
                </ul>
              </div>
            )}
          </>
        )}

        {/* Error display */}
        {(processingError || audioRecorder.error) && (
          <div className="error-message">
            <p>❌ {processingError || audioRecorder.error}</p>
            {processingError?.includes('Ollama') && (
              <p style={{ fontSize: '11px', marginTop: '8px', color: '#aaa' }}>
                Make sure Ollama is running. Start it from the command line or check if it's running on port 11434.
              </p>
            )}
          </div>
        )}

        <TaskList
          tasks={tasks}
          onToggle={handleToggleTask}
          onDelete={handleDeleteTask}
          onUpdate={handleUpdateTask}
        />
        {hasWhisperModel === false && (
          <div className="model-warning">
            <p>⚠️ No Whisper model installed. Voice recording requires a model.</p>
            <button onClick={() => setShowSettings(true)}>Open Settings</button>
          </div>
        )}
      </div>
      <SettingsModal
        isOpen={showSettings}
        onClose={() => {
          setShowSettings(false);
          checkWhisperModels(); // Refresh model status after closing
        }}
        availableDevices={audioRecorder.availableDevices}
        selectedDeviceId={audioRecorder.selectedDeviceId}
        onDeviceChange={audioRecorder.setSelectedDeviceId}
        onRefreshDevices={audioRecorder.refreshDevices}
      />
    </div>
  );
}

export default App;
