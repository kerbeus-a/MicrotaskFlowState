# FlowState

A lightweight Windows desktop application for high-intensity knowledge workers. FlowState uses voice-to-text and local LLMs to manage micro-tasks, allowing users to "dump" their current status and future intentions via voice.

## Features

- 🎤 **Voice Logging**: Record voice notes that are automatically transcribed and parsed
- 🤖 **AI Task Parsing**: Local LLM (Ollama) analyzes transcripts to extract completed and new tasks
- ✅ **Task Management**: Simple, minimalist task list with manual editing capabilities
- ⏰ **Awareness Timer**: 15-minute rhythmic timer to break hyperfocus and check current tasks
- 🔒 **100% Local**: All processing happens on your machine - no data leaves your computer
- ⌨️ **Global Hotkey**: Win + Alt + R to quickly start recording

## Tech Stack

- **Frontend**: React + TypeScript + Vite
- **Backend**: Rust + Tauri
- **Database**: SQLite
- **Speech-to-Text**: Whisper.cpp (to be integrated)
- **Local LLM**: Ollama (default) or LM Studio

## Prerequisites

Before running this project, you need:

1. **Node.js** (v18 or higher) and npm
2. **Rust** (latest stable version) - [Install Rust](https://www.rust-lang.org/tools/install)
3. **Ollama** - [Install Ollama](https://ollama.ai/) and pull a model:
   ```bash
   ollama pull llama3
   # or
   ollama pull mistral
   ```
4. **Whisper Models** - Models are automatically downloaded through the app's Settings UI (no manual installation needed!)

## Installation

1. Clone the repository:
   ```bash
   git clone <repository-url>
   cd MicroTask
   ```

2. Install frontend dependencies:
   ```bash
   npm install
   ```

3. The Rust dependencies will be automatically installed when you first build the project.

## Development

Run the development server:

```bash
npm run tauri dev
```

This will:
- Start the Vite dev server for the React frontend
- Build and run the Tauri application
- Enable hot-reload for both frontend and backend changes

## Building

To build the application:

```bash
npm run tauri build
```

The built application will be in `src-tauri/target/release/`.

## Configuration

### Model Management

FlowState includes built-in model management:

1. **Whisper Models**: Click the ⚙️ settings button to open the Settings modal
   - Models are automatically downloaded from Hugging Face
   - Start with "Tiny" (75 MB) for fastest performance
   - Larger models (Base, Small, Medium, Large) provide better accuracy
   - Models are stored in your app data directory
   - Download progress is shown in real-time

2. **Ollama Settings**:
   - By default, FlowState connects to Ollama at `http://localhost:11434` using the `llama3` model
   - You can override these by setting environment variables:
     - `OLLAMA_URL`: Custom Ollama server URL (default: `http://localhost:11434`)
     - `OLLAMA_MODEL`: Model name to use (default: `llama3`)
   - Ollama status is shown in the Settings modal

### Window Settings

The window is configured to be 300px wide by default. You can modify this in `src-tauri/tauri.conf.json`.

## Project Structure

```
.
├── src/                    # React frontend
│   ├── components/         # React components
│   ├── App.tsx            # Main app component
│   └── main.tsx           # Entry point
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── main.rs        # Tauri entry point
│   │   ├── commands.rs    # Tauri command handlers
│   │   ├── database.rs    # SQLite database operations
│   │   ├── timer.rs       # Awareness timer logic
│   │   ├── whisper.rs    # Whisper.cpp integration (placeholder)
│   │   └── ollama.rs      # Ollama API integration
│   ├── Cargo.toml         # Rust dependencies
│   └── tauri.conf.json    # Tauri configuration
└── package.json           # Node.js dependencies
```

## Usage

1. **Start the application**: Run `npm run tauri dev` or launch the built executable
2. **Record a voice log**: Click the record button or press `Win + Alt + R`
3. **View tasks**: Your tasks will appear in the list, automatically parsed from your voice logs
4. **Manage tasks**: 
   - Click checkbox to mark complete
   - Double-click to edit task text
   - Click × to delete
5. **Timer**: The 15-minute awareness timer runs automatically and will alert you when it expires

## Roadmap

### V1.1
- Integration with Outlook/Google Calendar
- Daily "big tasks" import

### V1.2
- Context-aware "End of Day" summary
- AI-generated summaries based on voice logs

### V2.0
- Mobile companion app
- Local Wi-Fi sync

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

[Add your license here]

## Notes

- **Whisper Models**: Models are automatically downloaded when you use the Settings UI. No manual installation required!
- **Ollama**: Make sure Ollama is running before using voice logging features. The app will show a warning if Ollama is not connected.
- **Model Storage**: Whisper models are stored in your app data directory (typically `%APPDATA%\com.flowstate.app\whisper_models` on Windows).
- **Whisper.cpp Binary**: The app will look for a `whisper` binary in your PATH. For full speech-to-text functionality, you'll need to install whisper.cpp separately or integrate it as a library.
- **Awareness Timer**: Currently uses a simple polling mechanism. For production, consider using Windows session unlock events (WTSSESSION_UNLOCK).
