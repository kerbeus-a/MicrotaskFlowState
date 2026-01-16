import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import ModelManager from "./ModelManager";
import "./SettingsModal.css";

interface AudioDevice {
  deviceId: string;
  label: string;
}

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  // Microphone settings
  availableDevices?: AudioDevice[];
  selectedDeviceId?: string | null;
  onDeviceChange?: (deviceId: string) => void;
  onRefreshDevices?: () => void;
}

export default function SettingsModal({ 
  isOpen, 
  onClose,
  availableDevices = [],
  selectedDeviceId,
  onDeviceChange,
  onRefreshDevices,
}: SettingsModalProps) {
  const [ollamaStatus, setOllamaStatus] = useState<"checking" | "connected" | "disconnected">("checking");
  const [ollamaModel, setOllamaModel] = useState<string>("llama3");
  const [autoStartEnabled, setAutoStartEnabled] = useState<boolean>(false);
  const [autoStartLoading, setAutoStartLoading] = useState<boolean>(false);
  const [timerDuration, setTimerDuration] = useState<number>(15);
  const [timerDurationLoading, setTimerDurationLoading] = useState<boolean>(false);
  const timerSaveTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  useEffect(() => {
    if (isOpen) {
      checkOllamaStatus();
      checkAutoStartStatus();
      loadTimerDuration();
    }
  }, [isOpen]);

  const loadTimerDuration = async () => {
    try {
      const duration = await invoke<number>("get_timer_duration");
      setTimerDuration(duration);
    } catch (error) {
      console.error("Failed to load timer duration:", error);
    }
  };

  const handleTimerDurationChange = (minutes: number) => {
    // Update local state immediately for smooth slider movement
    setTimerDuration(minutes);
    
    // Clear any pending save operation
    if (timerSaveTimeoutRef.current) {
      clearTimeout(timerSaveTimeoutRef.current);
    }
    
    // Debounce the save operation - only save after user stops sliding
    // Don't set loading state - keep slider enabled for smooth interaction
    timerSaveTimeoutRef.current = setTimeout(async () => {
      try {
        await invoke("set_timer_duration", { minutes });
        // Reset timer with new duration
        await invoke("reset_timer");
      } catch (error) {
        console.error("Failed to set timer duration:", error);
      }
    }, 500); // Wait 500ms after user stops sliding
  };

  const checkAutoStartStatus = async () => {
    try {
      const enabled = await invoke<boolean>("get_autostart_enabled");
      setAutoStartEnabled(enabled);
    } catch (error) {
      console.error("Failed to check auto-start status:", error);
    }
  };

  const toggleAutoStart = async () => {
    setAutoStartLoading(true);
    try {
      await invoke("set_autostart_enabled", { enabled: !autoStartEnabled });
      setAutoStartEnabled(!autoStartEnabled);
    } catch (error) {
      console.error("Failed to toggle auto-start:", error);
    }
    setAutoStartLoading(false);
  };

  const checkOllamaStatus = async () => {
    try {
      // Try to connect to Ollama
      const response = await fetch("http://localhost:11434/api/tags");
      if (response.ok) {
        setOllamaStatus("connected");
        const data = await response.json();
        if (data.models && data.models.length > 0) {
          setOllamaModel(data.models[0].name);
        }
      } else {
        setOllamaStatus("disconnected");
      }
    } catch (error) {
      setOllamaStatus("disconnected");
    }
  };

  if (!isOpen) return null;

  return (
    <div className="settings-modal-overlay" onClick={onClose}>
      <div className="settings-modal" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h2>Settings</h2>
          <button className="close-button" onClick={onClose}>×</button>
        </div>

        <div className="settings-content">
          <div className="settings-section">
            <h3>Microphone</h3>
            <div className="microphone-settings">
              <div className="device-select-container">
                <select
                  className="device-select"
                  value={selectedDeviceId || ''}
                  onChange={(e) => onDeviceChange?.(e.target.value)}
                >
                  {availableDevices.length === 0 ? (
                    <option value="">No microphones found</option>
                  ) : (
                    availableDevices.map((device) => (
                      <option key={device.deviceId} value={device.deviceId}>
                        {device.label}
                      </option>
                    ))
                  )}
                </select>
                <button 
                  className="refresh-devices-button"
                  onClick={onRefreshDevices}
                  title="Refresh device list"
                >
                  🔄
                </button>
              </div>
              {availableDevices.length === 0 && (
                <p className="status-detail">
                  No microphones detected. Please connect a microphone and click refresh.
                </p>
              )}
            </div>
          </div>

          <div className="settings-section">
            <h3>Timer</h3>
            <div className="timer-setting">
              <label className="slider-label">
                <span className="slider-text">Awareness Timer Duration</span>
                <span className="slider-value">{timerDuration} min</span>
              </label>
              <input
                type="range"
                min="0"
                max="60"
                value={timerDuration}
                onInput={(e) => {
                  const value = parseInt((e.target as HTMLInputElement).value);
                  setTimerDuration(value);
                }}
                onChange={(e) => handleTimerDurationChange(parseInt(e.target.value))}
                className="timer-slider"
              />
              <p className="status-detail">
                Set the timer duration (0-60 minutes). Timer alerts you when it expires.
              </p>
            </div>
          </div>

          <div className="settings-section">
            <h3>Startup</h3>
            <div className="autostart-setting">
              <label className="toggle-label">
                <input
                  type="checkbox"
                  checked={autoStartEnabled}
                  onChange={toggleAutoStart}
                  disabled={autoStartLoading}
                />
                <span className="toggle-text">
                  Start with Windows
                </span>
              </label>
              <p className="status-detail">
                Launch MicroTask automatically when you log in
              </p>
            </div>
          </div>

          <div className="settings-section">
            <h3>Ollama (LLM)</h3>
            <div className="ollama-status">
              {ollamaStatus === "checking" && (
                <span className="status-text">Checking...</span>
              )}
              {ollamaStatus === "connected" && (
                <div>
                  <span className="status-text status-connected">✓ Connected</span>
                  <p className="status-detail">Model: {ollamaModel}</p>
                </div>
              )}
              {ollamaStatus === "disconnected" && (
                <div>
                  <span className="status-text status-disconnected">✗ Not connected</span>
                  <p className="status-detail">
                    Please install and start Ollama from{" "}
                    <a href="https://ollama.ai" target="_blank" rel="noopener noreferrer">
                      ollama.ai
                    </a>
                  </p>
                </div>
              )}
            </div>
          </div>

          <div className="settings-section">
            <h3>Whisper (Speech-to-Text)</h3>
            <ModelManager />
          </div>
        </div>
      </div>
    </div>
  );
}
