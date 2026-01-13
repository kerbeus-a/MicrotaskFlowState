import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import TaskList from "./components/TaskList";
import RecordButton from "./components/RecordButton";
import TimerBar from "./components/TimerBar";
import SettingsModal from "./components/SettingsModal";
import AudioVisualizer from "./components/AudioVisualizer";
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
  const [alwaysOnTop, setAlwaysOnTop] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [hasWhisperModel, setHasWhisperModel] = useState<boolean | null>(null);
  const [selectedModel, setSelectedModel] = useState<string>("tiny");
  const [processingError, setProcessingError] = useState<string | null>(null);

  // Audio recorder hook
  const audioRecorder = useAudioRecorder();

  // Debug: Log when microphone devices are available
  useEffect(() => {
    if (audioRecorder.availableDevices.length > 0) {
      console.log('Microphone devices available:', audioRecorder.availableDevices);
      console.log('Selected device:', audioRecorder.selectedDeviceId);
      console.log('Test Mic button should be visible when not recording');
    }
  }, [audioRecorder.availableDevices, audioRecorder.selectedDeviceId, audioRecorder.state.isRecording]);

  useEffect(() => {
    if (!isTauri) {
      console.warn("⚠️ Running in browser mode. Tauri features will not work. Please run 'npm run tauri dev' to launch the Tauri app.");
      return;
    }

    loadTasks();
    updateTimer();
    checkWhisperModels();
    const interval = setInterval(updateTimer, 1000);

    // Listen for timer alerts
    const unlisten = listen("timer-alert", () => {
      playChime();
      // Visual pulse effect
      document.body.style.animation = "pulse 0.5s";
      setTimeout(() => {
        document.body.style.animation = "";
      }, 500);
    });

    // Listen for start-recording event (from global shortcut)
    const unlistenRecording = listen("start-recording", () => {
      handleRecordClick();
    });

    return () => {
      clearInterval(interval);
      unlisten.then(fn => fn());
      unlistenRecording.then(fn => fn());
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
        console.log(`Using Whisper model: ${firstInstalled.name}`);
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

  const updateTimer = async () => {
    if (!isTauri) return;
    try {
      const remaining = await invoke<number>("get_timer_status");
      setTimerRemaining(remaining);
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
      const updatedTasks = await invoke<Task[]>("process_voice_recording", {
        audioData,
        modelName: selectedModel,
      });

      // Reload tasks to get the latest
      await loadTasks();

      // Show success feedback
      console.log("Voice recording processed successfully:", updatedTasks);
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
      const result = await invoke("set_always_on_top", { alwaysOnTop: newValue });
      console.log("Always on top result:", result);
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
      <TimerBar remaining={timerRemaining} />
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
              {alwaysOnTop ? "📌" : "📍"}
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

        {/* Current microphone indicator */}
        {audioRecorder.availableDevices.length > 0 && (
          <div className="current-microphone-container">
            <div className="current-microphone" onClick={() => setShowSettings(true)} title="Click to change microphone">
              🎤 {audioRecorder.availableDevices.find(d => d.deviceId === audioRecorder.selectedDeviceId)?.label || 'Select microphone'}
            </div>
          </div>
        )}

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
