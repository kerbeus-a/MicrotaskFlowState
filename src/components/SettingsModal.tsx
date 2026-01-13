import { useState, useEffect } from "react";
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

  useEffect(() => {
    if (isOpen) {
      checkOllamaStatus();
    }
  }, [isOpen]);

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
