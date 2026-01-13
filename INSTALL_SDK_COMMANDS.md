# Commands to Install Windows SDK

## Quick Check: Is SDK Already Installed?

```powershell
# Check if Windows SDK is installed
Test-Path "C:\Program Files (x86)\Windows Kits\10\Lib"
Test-Path "C:\Program Files\Windows Kits\10\Lib"

# List installed SDK versions
Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\Lib" -Directory | Select-Object Name
```

## Method 1: Via Visual Studio Installer (Recommended)

### Open Visual Studio Installer:
```powershell
& "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vs_installer.exe"
```

### Or use the script:
```powershell
.\install-windows-sdk.ps1
```

### Manual Steps in Visual Studio Installer:
1. Click **Modify** on Visual Studio Build Tools 2022
2. Ensure **"Desktop development with C++"** is checked
3. Under **"Individual components"**, search for **"Windows SDK"**
4. Check the latest version (e.g., **Windows 11 SDK (10.0.22621.0)**)
5. Click **Modify** to install

## Method 2: Install Standalone Windows SDK

### Download and Install:
```powershell
# Open download page
Start-Process "https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/"

# Or download directly (replace with latest version URL)
# Invoke-WebRequest -Uri "https://go.microsoft.com/fwlink/p/?linkid=2249371" -OutFile "$env:TEMP\winsdksetup.exe"
# Start-Process "$env:TEMP\winsdksetup.exe"
```

## Method 3: Using winget (Windows Package Manager)

If you have winget installed:
```powershell
# List available Windows SDK versions
winget search "Windows SDK"

# Install latest Windows SDK (example - check actual package name)
# winget install Microsoft.WindowsSDK
```

## Method 4: Using Chocolatey

If you have Chocolatey installed:
```powershell
# Search for Windows SDK
choco search windows-sdk

# Install (example - check actual package name)
# choco install windows-sdk
```

## Verify Installation

After installation, verify:
```powershell
# Check for dbghelp.lib (the missing library)
Get-ChildItem "C:\Program Files*" -Filter "dbghelp.lib" -Recurse -ErrorAction SilentlyContinue | Select-Object FullName

# Check SDK version
Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\Lib" -Directory | Sort-Object Name -Descending | Select-Object -First 1 Name
```

## After Installation

1. **Restart your terminal/IDE**
2. **Try building again:**
   ```powershell
   .\build-with-vs-env.ps1
   ```

## Troubleshooting

If SDK is installed but still not found:

1. **Check environment variables:**
   ```powershell
   $env:WindowsSdkDir
   $env:WindowsSDKVersion
   ```

2. **Manually set if needed:**
   ```powershell
   $sdkPath = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\Lib" -Directory | Sort-Object Name -Descending | Select-Object -First 1
   $env:LIB = "$($sdkPath.FullName)\um\x64;$env:LIB"
   ```
