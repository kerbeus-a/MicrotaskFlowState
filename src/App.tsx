import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { listen } from "@tauri-apps/api/event";
import TaskList from "./components/TaskList";
import RecordButton from "./components/RecordButton";
import TimerBar from "./components/TimerBar";
import SettingsModal from "./components/SettingsModal";
import "./App.css";

interface Task {
  id: number;
  text: string;
  completed: boolean;
  created_at: string;
  completed_at: string | null;
}

function App() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [isRecording, setIsRecording] = useState(false);
  const [timerRemaining, setTimerRemaining] = useState(900); // 15 minutes in seconds
  const [alwaysOnTop, setAlwaysOnTop] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [hasWhisperModel, setHasWhisperModel] = useState<boolean | null>(null);

  useEffect(() => {
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
    try {
      const models = await invoke<Array<{ installed: boolean }>>("list_whisper_models");
      const hasModel = models.some(m => m.installed);
      setHasWhisperModel(hasModel);
      
      // Show settings if no model is installed
      if (!hasModel) {
        setTimeout(() => setShowSettings(true), 1000);
      }
    } catch (error) {
      console.error("Failed to check Whisper models:", error);
    }
  };

  const loadTasks = async () => {
    try {
      const loadedTasks = await invoke<Task[]>("get_tasks");
      setTasks(loadedTasks);
    } catch (error) {
      console.error("Failed to load tasks:", error);
    }
  };

  const updateTimer = async () => {
    try {
      const remaining = await invoke<number>("get_timer_status");
      setTimerRemaining(remaining);
    } catch (error) {
      console.error("Failed to get timer status:", error);
    }
  };

  const handleRecordClick = async () => {
    if (isRecording) {
      // Stop recording
      setIsRecording(false);
      // TODO: Stop audio recording and process
      // For now, this is a placeholder
    } else {
      // Start recording
      setIsRecording(true);
      // TODO: Start audio recording
      // For now, this is a placeholder
    }
  };

  const handleProcessTranscript = async (transcript: string) => {
    try {
      const updatedTasks = await invoke<Task[]>("process_voice_log", {
        transcript,
      });
      await loadTasks();
      
      // Show feedback
      if (updatedTasks.length > 0) {
        // Brief highlight of changes
        console.log("Tasks updated:", updatedTasks);
      }
    } catch (error) {
      console.error("Failed to process transcript:", error);
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
    try {
      const newValue = !alwaysOnTop;
      await invoke("set_always_on_top", { alwaysOnTop: newValue });
      setAlwaysOnTop(newValue);
    } catch (error) {
      console.error("Failed to toggle always on top:", error);
    }
  };

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
          isRecording={isRecording}
          onClick={handleRecordClick}
        />
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
      />
    </div>
  );
}

export default App;
