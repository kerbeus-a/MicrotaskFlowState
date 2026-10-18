# FlowState

A lightweight native Windows desktop application for high-intensity knowledge workers. FlowState uses voice-to-text and local LLMs to manage micro-tasks, allowing users to dump current status and future intentions via voice.

## Features

- Voice logging with local Whisper transcription
- Local task parsing with the built-in parser or Ollama
- Simple task management with manual entry/editing
- Awareness timer for periodic check-ins
- Local SQLite storage
- Verified Whisper model downloads with pinned SHA-256 hashes

## Tech Stack

- **UI**: Rust + egui/eframe
- **Database**: SQLite
- **Speech-to-Text**: whisper.cpp via `whisper-rs`
- **Local LLM**: Ollama

## Prerequisites

1. **Rust** latest stable: <https://www.rust-lang.org/tools/install>
2. **Windows build tools** with the Windows SDK.
3. **Ollama** if you want LLM-assisted parsing:
   ```bash
   ollama pull llama3.2
   ```

## Development

From the Rust project directory:

```bash
cd rust
cargo run --bin flowstate
```

## Building

For a Windows release build:

```bash
cd rust
cargo build --release --bin flowstate
```

The executable is written to `rust/target/release/flowstate.exe`.

## Configuration

### Model Management

Whisper models are downloaded from Hugging Face through the settings UI. Downloads are written to a temporary file, checked against expected byte size and SHA-256 hash, then atomically installed.

Available models:

- Tiny
- Base
- Small
- Medium

### Ollama

By default, FlowState connects to `http://localhost:11434` using `llama3.2`.

Environment overrides:

- `OLLAMA_URL`: custom Ollama server URL
- `OLLAMA_MODEL`: model name to use

## Project Structure

```
.
├── rust/                  # Rust native application
│   ├── src/
│   │   ├── database.rs    # SQLite database operations
│   │   ├── native_main.rs # egui application entry point
│   │   ├── whisper.rs    # Whisper model management and transcription
│   │   └── ollama.rs      # Ollama API integration
│   ├── icons/
│   ├── Cargo.toml
│   └── build.rs
```

## Usage

1. Start the application with `cargo run --bin flowstate` or launch the built executable.
2. Download a Whisper model from settings.
3. Record a voice log.
4. Review and edit parsed tasks.
5. Use the awareness timer for periodic check-ins.

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

- Whisper models are stored in the FlowState app data directory.
- Ollama is optional; the built-in parser is used when Ollama is disabled or unavailable.
