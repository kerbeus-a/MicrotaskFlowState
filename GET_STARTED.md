# Get Started with FlowState Development

## Quick Setup (5 minutes)

### Step 1: Install Rust ⚠️ REQUIRED

Rust is needed to build the Tauri backend. Choose one method:

**Method A: Use the automated script**
```powershell
powershell -ExecutionPolicy Bypass -File .\install-rust.ps1
```

**Method B: Manual installation**
1. Visit https://rustup.rs/
2. Download and run the installer
3. Accept default settings
4. **Close and reopen your terminal** after installation

**Verify:**
```powershell
rustc --version
cargo --version
```

### Step 2: Install Ollama (Optional)

Ollama is needed for AI task parsing. If you skip this, the app will show a warning.

1. Download from https://ollama.ai/
2. Install and start Ollama
3. Pull a model:
   ```powershell
   ollama pull llama3
   ```

### Step 3: Run the App

```powershell
npm run tauri dev
```

**First run will:**
- Download Rust dependencies (5-10 minutes, one-time)
- Build the Rust backend
- Launch the app
- Check for Whisper models (you can download one in Settings)

## Check Your Setup

Run the prerequisites checker:
```powershell
powershell -ExecutionPolicy Bypass -File .\check-prerequisites.ps1
```

## What's Already Done ✅

- ✅ Node.js dependencies installed (`npm install` completed)
- ✅ Project structure created
- ✅ All source files in place
- ✅ Configuration files ready

## What You Need to Do

1. **Install Rust** (required) - See Step 1 above
2. **Install Ollama** (optional) - See Step 2 above
3. **Run the app** - `npm run tauri dev`

## Troubleshooting

### "rustc is not recognized"
- Rust isn't installed or not in PATH
- Install Rust (see Step 1)
- **Important**: Close and reopen terminal after installing

### "Failed to build Tauri"
- You may need Visual Studio Build Tools
- Tauri will guide you if needed
- See: https://tauri.app/v1/guides/getting-started/prerequisites

### "Ollama not connected"
- This is OK for now - you can test other features
- Install Ollama when ready to test AI features

## Development Workflow

1. **Start dev server:**
   ```powershell
   npm run tauri dev
   ```

2. **Make changes:**
   - Frontend (React): Changes hot-reload automatically
   - Backend (Rust): Requires rebuild (automatic in dev mode)

3. **Build for production:**
   ```powershell
   npm run tauri build
   ```

## Next Steps After Setup

1. Test the timer functionality
2. Download a Whisper model (Settings → Whisper Models)
3. Test task management
4. Implement voice recording (when ready)

## Need Help?

- See [SETUP.md](./SETUP.md) for detailed instructions
- See [PREPARATION_STATUS.md](./PREPARATION_STATUS.md) for current status
- Check [README.md](./README.md) for project overview
