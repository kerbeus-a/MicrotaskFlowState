# FlowState - Rust Installation Script for Windows
# This script will install Rust using rustup

Write-Host "FlowState - Installing Rust..." -ForegroundColor Cyan
Write-Host ""

# Check if rustup is already installed
if (Get-Command rustup -ErrorAction SilentlyContinue) {
    Write-Host "Rust is already installed!" -ForegroundColor Green
    rustup --version
    Write-Host ""
    Write-Host "Updating Rust toolchain..." -ForegroundColor Yellow
    rustup update
    exit 0
}

Write-Host "Downloading rustup installer..." -ForegroundColor Yellow

# Download rustup-init.exe
$rustupUrl = "https://win.rustup.rs/x86_64"
$rustupPath = "$env:TEMP\rustup-init.exe"

try {
    Invoke-WebRequest -Uri $rustupUrl -OutFile $rustupPath -UseBasicParsing
    Write-Host "Downloaded rustup installer to: $rustupPath" -ForegroundColor Green
} catch {
    Write-Host "Failed to download rustup installer: $_" -ForegroundColor Red
    Write-Host "Please download manually from: https://rustup.rs/" -ForegroundColor Yellow
    exit 1
}

Write-Host ""
Write-Host "Starting rustup installer..." -ForegroundColor Yellow
Write-Host "The installer will open in a new window." -ForegroundColor Yellow
Write-Host "Please follow the on-screen instructions." -ForegroundColor Yellow
Write-Host ""
Write-Host "Recommended settings:" -ForegroundColor Cyan
Write-Host "  - Default host triple: x86_64-pc-windows-msvc" -ForegroundColor White
Write-Host "  - Default toolchain: stable" -ForegroundColor White
Write-Host ""

# Start the installer
Start-Process -FilePath $rustupPath -Wait

# Check if installation was successful
Start-Sleep -Seconds 2

if (Get-Command rustc -ErrorAction SilentlyContinue) {
    Write-Host ""
    Write-Host "Rust installation successful!" -ForegroundColor Green
    Write-Host ""
    rustc --version
    cargo --version
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor Cyan
    Write-Host "  1. Close and reopen your terminal" -ForegroundColor White
    Write-Host "  2. Run: npm run tauri dev" -ForegroundColor White
} else {
    Write-Host ""
    Write-Host "Rust installation may not be complete." -ForegroundColor Yellow
    Write-Host "Please close and reopen your terminal, then verify with: rustc --version" -ForegroundColor Yellow
}
