# Fix for Windows Build Error: LNK1181 (dbghelp.lib not found)

## Problem
The linker cannot find `dbghelp.lib`, which is part of the Windows SDK.

## Solution 1: Install Windows SDK (Recommended)

1. Open **Visual Studio Installer**
2. Click **Modify** on your Visual Studio Build Tools 2022 installation
3. Ensure **"Desktop development with C++"** workload is selected
4. Under individual components, ensure **"Windows 10 SDK"** or **"Windows 11 SDK"** (latest version) is checked
5. Click **Modify** to install

After installation, restart your terminal and try building again:
```powershell
npm run tauri dev
```

## Solution 2: Use Build Script

Use the provided build script that automatically sets up the environment:

```powershell
.\build-with-vs-env.ps1
```

This script:
- Finds Visual Studio Build Tools
- Initializes the build environment
- Runs `npm run tauri dev` with proper environment variables

## Solution 3: Manual Environment Setup

If you prefer to set up the environment manually:

1. Open **Developer Command Prompt for VS 2022** (from Start Menu)
2. Navigate to your project:
   ```cmd
   cd E:\Dev_projects\MicroTask
   ```
3. Run the build:
   ```cmd
   npm run tauri dev
   ```

## Verify Installation

To check if Windows SDK is installed:
```powershell
Test-Path "C:\Program Files (x86)\Windows Kits\10\Lib"
```

If this returns `False`, the Windows SDK is not installed.

## Additional Notes

- The Windows SDK is required for building Rust projects that use Windows APIs
- Tauri requires the Windows SDK for its Windows-specific features
- After installing the SDK, you may need to restart your terminal/IDE
