# Node.js Installation Check Script

Write-Host "Checking Node.js installation..." -ForegroundColor Cyan

# Check if Node.js is installed
try {
    $nodeVersion = node --version 2>$null
    if ($nodeVersion) {
        Write-Host "✓ Node.js is installed: $nodeVersion" -ForegroundColor Green
    } else {
        Write-Host "✗ Node.js is NOT installed" -ForegroundColor Red
        Write-Host "  Download from: https://nodejs.org/" -ForegroundColor Yellow
        exit 1
    }
} catch {
    Write-Host "✗ Node.js is NOT installed" -ForegroundColor Red
    Write-Host "  Download from: https://nodejs.org/" -ForegroundColor Yellow
    exit 1
}

# Check if npm is installed
try {
    $npmVersion = npm --version 2>$null
    if ($npmVersion) {
        Write-Host "✓ npm is installed: $npmVersion" -ForegroundColor Green
    } else {
        Write-Host "✗ npm is NOT installed" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host "✗ npm is NOT installed" -ForegroundColor Red
    exit 1
}

# Check if in correct directory
$currentDir = (Get-Location).Path
if ($currentDir -like "*MicroTask*") {
    Write-Host "✓ You are in the MicroTask project directory" -ForegroundColor Green
} else {
    Write-Host "⚠ You may not be in the MicroTask project directory" -ForegroundColor Yellow
    Write-Host "  Current directory: $currentDir" -ForegroundColor Yellow
}

# Check if package.json exists
if (Test-Path "package.json") {
    Write-Host "✓ package.json found" -ForegroundColor Green

    # Check if node_modules exists
    if (Test-Path "node_modules") {
        Write-Host "✓ node_modules directory exists" -ForegroundColor Green
        Write-Host "`nYou can run: npm run tauri dev" -ForegroundColor Cyan
    } else {
        Write-Host "⚠ node_modules directory NOT found" -ForegroundColor Yellow
        Write-Host "`nPlease run: npm install" -ForegroundColor Cyan
    }
} else {
    Write-Host "✗ package.json NOT found in current directory" -ForegroundColor Red
}

Write-Host "`n=== Summary ===" -ForegroundColor Cyan
Write-Host "1. Install Node.js from https://nodejs.org/ (if not installed)" -ForegroundColor White
Write-Host "2. Restart your terminal after installation" -ForegroundColor White
Write-Host "3. Run: npm install" -ForegroundColor White
Write-Host "4. Run: npm run tauri dev" -ForegroundColor White
