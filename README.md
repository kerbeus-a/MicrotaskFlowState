# FlowState

A small, fast desktop app for capturing micro-tasks by voice. Tap the mic, say what's on your plate ("buy batteries, call the dentist, and the invoice is done"), and FlowState turns it into a task list. Everything runs on your machine: speech recognition and the optional AI parser are built in.

Runs on **Windows** and **Linux**.

## Features

- **Voice capture**: tap to talk, hold for push-to-talk, or press <kbd>Space</kbd>. <kbd>Esc</kbd> discards a recording.
- **On-device speech recognition**: whisper.cpp (Whisper Tiny/Base/Small/Medium, English), downloaded from the Settings screen.
- **Task understanding**, three choices:
  - *Quick rules*: instant keyword matching.
  - *On-device AI*: a small Qwen2.5 model (0.5B or 1.5B) running on the CPU. It splits rambling notes into separate tasks and recognises "X is done".
  - *Ollama*: any model on an Ollama server.
- **Tasks**: type to add; click a task's text to edit it (<kbd>Enter</kbd> saves, <kbd>Esc</kbd> cancels); round checkbox to complete; delete on hover; "Done" tab keeps the last 7 days.
- **Check-in timer** around the mic button, with chime, pause and restart; it also flashes the taskbar when time is up.
- Keep-on-top pin, remembered window size and position, local SQLite storage.

Model downloads are pinned to exact sizes and SHA-256 hashes, written to a temporary file and only installed after verification.

## Building

Requirements on all platforms: Rust (stable), CMake, and Clang/LLVM (for whisper.cpp bindings).

### Windows

1. Install [Rust](https://rustup.rs), Visual Studio Build Tools (Desktop C++ with the Windows SDK), [CMake](https://cmake.org/download/) and [LLVM](https://github.com/llvm/llvm-project/releases) (sets `LIBCLANG_PATH`).
2. Build:
   ```bash
   cd rust
   cargo build --release
   ```
   The app is `rust/target/release/flowstate.exe`, a single file with its icon embedded.

### Linux (Debian/Ubuntu)

```bash
sudo apt install build-essential cmake clang libclang-dev pkg-config \
  libasound2-dev libxkbcommon-dev libwayland-dev libx11-dev \
  libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev
cd rust
cargo build --release
```

The binary is `rust/target/release/flowstate`. For a menu entry, copy `rust/packaging/linux/flowstate.desktop` to `~/.local/share/applications/` and put `flowstate` on your `PATH`.

### Fedora (.rpm)

Easiest: download the `flowstate-fedora-rpm` artifact from the latest CI run and install it:

```bash
sudo dnf install ./flowstate-*.x86_64.rpm
```

Or build the package yourself on Fedora (installs build dependencies, then writes the .rpm to `rust/target/generate-rpm/`):

```bash
bash rust/packaging/linux/build-rpm.sh
```

The package installs `/usr/bin/flowstate`, a menu entry and the icon.

CI builds and tests Windows, Ubuntu and Fedora on every push (`.github/workflows/build.yml`).

### Performance notes

- `rust/.cargo/config.toml` enables AVX2/FMA for x86-64 (any CPU since 2013). Without it the on-device AI is ~10× slower.
- `Cargo.toml` contains a `build-override` that keeps whisper.cpp optimized in release builds on MSVC.

## Usage

1. Launch FlowState and click **Download Whisper Base** in the banner (148 MB, one time).
2. Optional: in **Settings → Task understanding**, choose *On-device AI* and download Qwen2.5 0.5B (491 MB). The model loads in the background at startup, so each memo then takes about a second.
3. Tap the mic and talk.

### Sync with iPhone (Nextcloud)

FlowState can sync two ways with a Nextcloud task list, which the iPhone's built-in Reminders app shows as a list.

1. **Nextcloud:** go to *Settings → Security → Devices & sessions* and create two app passwords: "FlowState" and "iPhone".
2. **FlowState (each computer):** open *Settings → Sync with Nextcloud*, enter your server address, username, the FlowState app password and a list name (default `FlowState`), then click **Connect**. The list is created if it doesn't exist.
3. **iPhone:** go to *Settings → Apps → Reminders → Reminders Accounts → Add Account → Other → Add CalDAV Account* and enter:
   - Server: your Nextcloud address, e.g. `cloud.example.com`
   - User name: your username
   - Password: the iPhone app password

   The **FlowState** list then appears in Reminders. Add to it by typing, or tell Siri "add *buy filters* to FlowState".

What syncs: task text, done/not done, and high priority (`!!!` in Reminders). Notes, due dates and alerts added on the phone are kept, though FlowState doesn't show them. Deleting a task on one side deletes it on the other. Changes made on the computer go up within a second or two; the server is checked every 30 seconds.

The app password is stored in FlowState's local database (`flowstate.db`).

### Command-line diagnostics

```bash
flowstate --download whisper-base.en         # or whisper-tiny.en, qwen2.5-0.5b, qwen2.5-1.5b, ...
flowstate --transcribe memo.wav builtin      # transcribe a WAV and show the parsed tasks (rules|builtin)
```

## Configuration

Settings are saved automatically. Data lives in:

- Windows: `%APPDATA%\flowstate\` (`flowstate.db`, `models\`)
- Linux: `~/.local/share/flowstate/`

Environment variables, used as defaults until changed in Settings:

- `WHISPER_URL`: a whisper.cpp server `/inference` endpoint (switches speech recognition to *Remote server*)
- `OLLAMA_URL` (default `http://localhost:11434`), `OLLAMA_MODEL` (default `llama3.2`)

## Project structure

```
rust/
├── build.rs            # renders the icon and embeds it in the Windows .exe
├── packaging/linux/    # .desktop entry
└── src/
    ├── main.rs         # entry point, window setup, CLI diagnostics
    ├── app.rs          # UI and app state
    ├── theme.rs        # colors, typography, egui style
    ├── icon.rs         # procedural app icon (shared with build.rs)
    ├── audio.rs        # microphone capture, resampling, chime
    ├── whisper.rs      # on-device and remote speech-to-text
    ├── llm.rs          # built-in language model runner (candle) with prompt caching
    ├── qwen2.rs        # Qwen2 model (vendored from candle-transformers, + Clone)
    ├── parser.rs       # transcript → tasks (rules, built-in AI, Ollama)
    ├── models.rs       # model catalog and verified downloader
    └── database.rs     # SQLite tasks and settings
```

## Roadmap

- Calendar integration (Outlook/Google) and daily "big tasks" import
- End-of-day summary from voice logs
- Mobile companion app with local Wi-Fi sync
