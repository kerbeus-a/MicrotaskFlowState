import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import ModelManager from "./ModelManager";
import "./SettingsModal.css";

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export default function SettingsModal({ isOpen, onClose }: SettingsModalProps) {
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
