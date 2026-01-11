# Model Management Guide

FlowState uses a hybrid approach for model management:

## Architecture

### Whisper Models (Speech-to-Text)
- **Bundled/Downloaded**: Models are downloaded automatically through the app
- **Storage**: Stored in app data directory (`%APPDATA%\com.flowstate.app\whisper_models`)
- **Available Models**:
  - **Tiny** (75 MB) - Fastest, good for most use cases
  - **Base** (142 MB) - Better accuracy, still fast
  - **Small** (466 MB) - Good balance
  - **Medium** (1.4 GB) - High accuracy
  - **Large** (2.9 GB) - Best accuracy, slower

### Ollama Models (LLM)
- **External Service**: Uses Ollama running as a separate process
- **No Bundling**: Models are managed by Ollama, not FlowState
- **Default**: `llama3` model
- **Configuration**: Set via environment variables or Ollama CLI

## How It Works

### First Run
1. App checks for Whisper models on startup
2. If no model is found, Settings modal opens automatically
3. User can download a model with one click
4. Download progress is shown in real-time

### Model Download
- Models are downloaded from Hugging Face CDN
- Progress is streamed and shown in the UI
- Models are validated after download
- Failed downloads can be retried

### Model Selection
- Users can select which model to use
- Multiple models can be installed simultaneously
- Models can be deleted to free up space
- Default recommendation: Start with "Tiny"

## Implementation Details

### Backend (Rust)
- `src-tauri/src/whisper.rs`: Model management logic
- `src-tauri/src/commands.rs`: Tauri commands for model operations
- Models downloaded using `reqwest` with streaming support
- Progress events emitted to frontend via Tauri events

### Frontend (React)
- `src/components/ModelManager.tsx`: Model management UI
- `src/components/SettingsModal.tsx`: Settings interface
- Real-time download progress display
- Model status indicators

## API Commands

### List Models
```typescript
const models = await invoke<ModelInfo[]>("list_whisper_models");
```

### Download Model
```typescript
await invoke("download_whisper_model", { modelName: "tiny" });
// Progress events: "model-download-progress"
```

### Check Model
```typescript
const exists = await invoke<boolean>("check_whisper_model", { modelName: "tiny" });
```

### Delete Model
```typescript
await invoke("delete_whisper_model", { modelName: "tiny" });
```

### Transcribe Audio
```typescript
const transcript = await invoke<string>("transcribe_audio", {
  audioPath: "/path/to/audio.wav",
  modelName: "tiny"
});
```

## Model URLs

Models are downloaded from Hugging Face:
- Tiny: `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin`
- Base: `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin`
- Small: `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin`
- Medium: `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin`
- Large: `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin`

## Benefits of This Approach

1. **Small Installer**: App stays lightweight (~20 MB)
2. **User Choice**: Users select model size based on their needs
3. **Easy Updates**: Models can be updated without reinstalling the app
4. **Flexibility**: Users can install multiple models and switch between them
5. **Offline Capable**: Once downloaded, models work offline
6. **Progressive Enhancement**: Start with small model, upgrade if needed

## Future Enhancements

- [ ] Model caching and validation
- [ ] Automatic model updates
- [ ] Model compression options
- [ ] Custom model support
- [ ] Model sharing between users
- [ ] Bandwidth-aware downloads
