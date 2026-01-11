# Quick Start Guide

Get FlowState up and running in 5 minutes!

## Step 1: Install Prerequisites

Make sure you have:
- ✅ Node.js 18+ ([Download](https://nodejs.org/))
- ✅ Rust ([Install](https://rustup.rs/))
- ✅ Ollama running ([Download](https://ollama.ai/))

## Step 2: Install Ollama Model

```bash
ollama pull llama3
```

## Step 3: Install Project Dependencies

```bash
npm install
```

## Step 4: Run the App

```bash
npm run tauri dev
```

That's it! The app should launch.

## First Use

1. **Test the timer**: The 15-minute awareness timer starts automatically
2. **Try the global shortcut**: Press `Win + Alt + R` to trigger recording (UI will be implemented)
3. **Add a task manually**: Double-click in the task area (once UI is complete)

## Current Status

✅ Project structure created
✅ Database setup complete
✅ Basic UI components
✅ Timer functionality
✅ Ollama integration
⏳ Voice recording (needs Whisper.cpp integration)
⏳ Icons (add to `src-tauri/icons/`)

## Next Development Steps

1. Implement actual voice recording (currently placeholder)
2. Add icons for the app
3. Test Ollama integration with real transcripts
4. Polish UI/UX

For detailed setup instructions, see [SETUP.md](./SETUP.md)
