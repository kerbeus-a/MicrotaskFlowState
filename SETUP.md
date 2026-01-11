# FlowState Setup Guide

This guide will help you set up the FlowState development environment.

## Prerequisites Installation

### 1. Install Node.js and npm

Download and install Node.js (v18 or higher) from [nodejs.org](https://nodejs.org/).

Verify installation:
```bash
node --version
npm --version
```

### 2. Install Rust

Install Rust using rustup from [rustup.rs](https://rustup.rs/) or run:

**Windows (PowerShell):**
```powershell
Invoke-WebRequest https://win.rustup.rs/x86_64 -OutFile rustup-init.exe
.\rustup-init.exe
```

**Or use the installer from the website.**

Verify installation:
```bash
rustc --version
cargo --version
```

### 3. Install Ollama

1. Download Ollama from [ollama.ai](https://ollama.ai/)
2. Install and start Ollama
3. Pull a model (choose one):
   ```bash
   ollama pull llama3
   # or
   ollama pull mistral
   # or
   ollama pull llama3.2
   ```

Verify Ollama is running:
```bash
curl http://localhost:11434/api/tags
```

### 4. (Optional) Install Whisper.cpp

For local speech-to-text, you'll need to set up Whisper.cpp:

1. Clone the repository:
   ```bash
   git clone https://github.com/ggerganov/whisper.cpp.git
   cd whisper.cpp
   ```

2. Follow the build instructions for your platform
3. Download a model (e.g., `base` or `small`)
4. Note the path to the whisper binary and model for later configuration

## Project Setup

### 1. Install Dependencies

```bash
npm install
```

This will install:
- React and TypeScript dependencies
- Tauri CLI
- Vite build tool

### 2. Verify Rust Dependencies

The Rust dependencies will be automatically downloaded when you first build. However, you can verify Cargo is working:

```bash
cd src-tauri
cargo check
cd ..
```

### 3. Configure Environment (Optional)

Create a `.env` file in the root directory (optional):

```env
OLLAMA_URL=http://localhost:11434
OLLAMA_MODEL=llama3
```

## Running the Application

### Development Mode

```bash
npm run tauri dev
```

This will:
- Start the Vite dev server
- Build the Rust backend
- Launch the Tauri application
- Enable hot-reload

### Building for Production

```bash
npm run tauri build
```

The built application will be in:
- Windows: `src-tauri/target/release/flowstate.exe`
- The installer will be in `src-tauri/target/release/bundle/`

## Troubleshooting

### Issue: "Failed to connect to Ollama"

**Solution:** Make sure Ollama is running:
```bash
# Check if Ollama is running
curl http://localhost:11434/api/tags

# If not, start Ollama (it should start automatically on Windows)
```

### Issue: Rust compilation errors

**Solution:** 
1. Make sure you have the latest Rust toolchain:
   ```bash
   rustup update
   ```
2. Make sure you're using the stable channel:
   ```bash
   rustup default stable
   ```

### Issue: Tauri build fails

**Solution:**
1. Make sure you have the required system dependencies for Tauri
2. On Windows, you may need Visual Studio Build Tools or the full Visual Studio with C++ support
3. See [Tauri prerequisites](https://tauri.app/v1/guides/getting-started/prerequisites)

### Issue: Global shortcut not working

**Solution:**
- Make sure no other application is using `Win + Alt + R`
- On Windows, you may need to run the app with administrator privileges (not recommended for development)
- The shortcut is registered when the app starts

### Issue: Database errors

**Solution:**
- The database is created automatically in the app data directory
- On Windows: `%APPDATA%\com.flowstate.app\flowstate.db`
- If you encounter errors, you can delete the database file and restart the app

## Next Steps

1. **Test the basic functionality:**
   - Launch the app
   - Try adding a task manually (you'll need to implement the UI for this first)
   - Test the timer

2. **Implement voice recording:**
   - Integrate Whisper.cpp for speech-to-text
   - Or use a web-based API for development (not recommended for production)

3. **Customize the UI:**
   - Modify colors and styles in `src/App.css` and component CSS files
   - Adjust window size in `src-tauri/tauri.conf.json`

4. **Add icons:**
   - Create or download icons
   - Place them in `src-tauri/icons/`
   - Required sizes: 32x32, 128x128, 256x256, icon.ico, icon.icns

## Development Tips

- Use `console.log()` in the frontend for debugging
- Use `eprintln!()` or `println!()` in Rust (visible in terminal when running `tauri dev`)
- Check browser DevTools: Right-click in the app → Inspect Element
- The app window can be resized during development
- Hot-reload works for frontend changes; Rust changes require a rebuild

## Getting Help

- Check the [Tauri documentation](https://tauri.app/v1/)
- Check the [React documentation](https://react.dev/)
- Check the [Ollama documentation](https://github.com/ollama/ollama)
