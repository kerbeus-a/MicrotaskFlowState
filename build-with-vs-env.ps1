# Script to build Tauri with Visual Studio environment properly configured
# This ensures Windows SDK libraries are found by the linker

Write-Host "Setting up Visual Studio Build Tools environment..." -ForegroundColor Cyan

# Try to find vcvarsall.bat in common locations
$vcvarsPaths = @(
    "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat",
    "C:\Program Files (x86)\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat",
    "C:\Program Files (x86)\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvarsall.bat",
    "C:\Program Files (x86)\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvarsall.bat"
)

$vcvarsPath = $null
foreach ($path in $vcvarsPaths) {
    if (Test-Path $path) {
        $vcvarsPath = $path
        break
    }
}

if (-not $vcvarsPath) {
    Write-Host "ERROR: Visual Studio Build Tools not found!" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please install one of the following:" -ForegroundColor Yellow
    Write-Host "  1. Visual Studio Build Tools 2022 (with C++ support)" -ForegroundColor White
    Write-Host "  2. Visual Studio 2022 Community/Professional/Enterprise (with C++ support)" -ForegroundColor White
    Write-Host ""
    Write-Host "Download from: https://visualstudio.microsoft.com/downloads/" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "During installation, make sure to select:" -ForegroundColor Yellow
    Write-Host "  - Desktop development with C++" -ForegroundColor White
    Write-Host "  - Windows 10/11 SDK (latest version)" -ForegroundColor White
    exit 1
}

# Check for Windows SDK
$sdkPaths = @(
    "C:\Program Files (x86)\Windows Kits\10\Lib",
    "C:\Program Files\Windows Kits\10\Lib"
)

$sdkFound = $false
foreach ($sdkPath in $sdkPaths) {
    if (Test-Path $sdkPath) {
        $sdkFound = $true
        Write-Host "Windows SDK found at: $sdkPath" -ForegroundColor Green
        break
    }
}

if (-not $sdkFound) {
    Write-Host "WARNING: Windows SDK not found in standard locations!" -ForegroundColor Yellow
    Write-Host "The build may fail. Please install Windows SDK via Visual Studio Installer." -ForegroundColor Yellow
    Write-Host ""
}

# Initialize Visual Studio environment and run the build
Write-Host "Initializing build environment and running Tauri dev..." -ForegroundColor Green
Write-Host ""

# Use cmd to run vcvarsall and then npm run tauri dev
# The && ensures npm only runs if vcvarsall succeeds
cmd /c "`"$vcvarsPath`" x64 >nul 2>&1 && npm run tauri dev"
