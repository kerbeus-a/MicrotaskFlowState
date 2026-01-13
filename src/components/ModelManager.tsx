import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./ModelManager.css";

// Check if running in Tauri (v2 uses __TAURI_INTERNALS__)
const isTauri = typeof window !== "undefined" && ("__TAURI_INTERNALS__" in window || "__TAURI_IPC__" in window);

interface ModelInfo {
  name: string;
  filename: string;
  size_mb: number;
  installed: boolean;
}

interface DownloadProgress {
  model: string;
  downloaded: number;
  total: number;
  progress: number;
}

export default function ModelManager() {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<DownloadProgress | null>(null);
  const [selectedModel, setSelectedModel] = useState<string>("tiny");

  useEffect(() => {
    if (!isTauri) {
      console.warn("ModelManager: Not running in Tauri, models cannot be loaded");
      return;
    }

    loadModels();

    // Listen for download progress
    const unlisten = listen("model-download-progress", (event) => {
      setDownloadProgress(event.payload as DownloadProgress);
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  const loadModels = async () => {
    if (!isTauri) return;
    try {
      const loadedModels = await invoke<ModelInfo[]>("list_whisper_models");
      console.log("Loaded models:", loadedModels);
      setModels(loadedModels);
      
      // Set default selected model to first installed one, or tiny
      const installed = loadedModels.find(m => m.installed);
      if (installed) {
        setSelectedModel(installed.name.toLowerCase());
      }
    } catch (error) {
      console.error("Failed to load models:", error);
      // Show error to user
      alert(`Failed to load models: ${error}. Please check the console for details.`);
    }
  };

  const handleDownload = async (modelName: string) => {
    if (!isTauri) {
      alert("Model download only works in the Tauri app. Please run 'npm run tauri dev'.");
      return;
    }
    try {
      setDownloading(modelName);
      setDownloadProgress(null);
      await invoke("download_whisper_model", { modelName: modelName.toLowerCase() });
      await loadModels();
      setDownloading(null);
      setDownloadProgress(null);
    } catch (error) {
      console.error("Failed to download model:", error);
      alert(`Failed to download model: ${error}`);
      setDownloading(null);
      setDownloadProgress(null);
    }
  };

  const handleDelete = async (modelName: string) => {
    if (!confirm(`Are you sure you want to delete ${modelName}?`)) {
      return;
    }

    try {
      await invoke("delete_whisper_model", { modelName: modelName.toLowerCase() });
      await loadModels();
    } catch (error) {
      console.error("Failed to delete model:", error);
      alert(`Failed to delete model: ${error}`);
    }
  };

  const formatSize = (mb: number) => {
    if (mb < 1000) {
      return `${mb} MB`;
    }
    return `${(mb / 1000).toFixed(1)} GB`;
  };

  if (!isTauri) {
    return (
      <div className="model-manager">
        <h2>Whisper Models</h2>
        <p style={{ color: "#f87171", padding: "20px", background: "#2a2a2a", borderRadius: "8px" }}>
          ⚠️ Model management requires the Tauri app. Please run 'npm run tauri dev' to access this feature.
        </p>
      </div>
    );
  }

  return (
    <div className="model-manager">
      <h2>Whisper Models</h2>
      <p className="model-manager-description">
        Download Whisper models for speech-to-text. Start with "Tiny" (75 MB) for fastest performance.
      </p>

      {models.length === 0 && (
        <div className="loading-models">Loading models...</div>
      )}

      <div className="models-list">
        {models.map((model) => (
          <div key={model.name} className="model-item">
            <div className="model-info">
              <div className="model-header">
                <span className="model-name">{model.name}</span>
                <span className="model-size">{formatSize(model.size_mb)}</span>
              </div>
              <div className="model-status">
                {model.installed ? (
                  <span className="status-installed">✓ Installed</span>
                ) : (
                  <span className="status-missing">✗ Not installed</span>
                )}
              </div>
            </div>
            <div className="model-actions">
              {model.installed ? (
                <>
                  <button
                    className="btn-select"
                    onClick={() => setSelectedModel(model.name.toLowerCase())}
                    disabled={selectedModel === model.name.toLowerCase()}
                  >
                    {selectedModel === model.name.toLowerCase() ? "Selected" : "Select"}
                  </button>
                  <button
                    className="btn-delete"
                    onClick={() => handleDelete(model.name)}
                  >
                    Delete
                  </button>
                </>
              ) : (
                <button
                  className="btn-download"
                  onClick={() => handleDownload(model.name)}
                  disabled={downloading !== null}
                >
                  {downloading === model.name ? "Downloading..." : "Download"}
                </button>
              )}
            </div>
            {downloading === model.name && downloadProgress && (
              <div className="download-progress">
                <div className="progress-bar">
                  <div
                    className="progress-fill"
                    style={{ width: `${downloadProgress.progress}%` }}
                  />
                </div>
                <div className="progress-text">
                  {formatSize(downloadProgress.downloaded)} / {formatSize(downloadProgress.total)} 
                  ({downloadProgress.progress}%)
                </div>
              </div>
            )}
          </div>
        ))}
      </div>

      <div className="model-note">
        <p>
          <strong>Note:</strong> Larger models provide better accuracy but require more disk space and processing time.
          For most users, "Tiny" or "Base" models work well.
        </p>
      </div>
    </div>
  );
}
