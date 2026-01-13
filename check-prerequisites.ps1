# FlowState - Prerequisites Check Script
# This script checks if all required tools are installed

Write-Host "FlowState - Checking Prerequisites..." -ForegroundColor Cyan
Write-Host ""

$allGood = $true

# Check Node.js
Write-Host "Checking Node.js..." -ForegroundColor Yellow
try {
    $nodeVersion = node --version
    Write-Host "  ✓ Node.js: $nodeVersion" -ForegroundColor Green
} catch {
    Write-Host "  ✗ Node.js: NOT INSTALLED" -ForegroundColor Red
    Write-Host "    Download from: https://nodejs.org/" -ForegroundColor Yellow
    $allGood = $false
}

# Check npm
Write-Host "Checking npm..." -ForegroundColor Yellow
try {
    $npmVersion = npm --version
    Write-Host "  ✓ npm: $npmVersion" -ForegroundColor Green
} catch {
    Write-Host "  ✗ npm: NOT INSTALLED" -ForegroundColor Red
    $allGood = $false
}

# Check Rust
Write-Host "Checking Rust..." -ForegroundColor Yellow
try {
    $rustVersion = rustc --version 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ✓ Rust: $rustVersion" -ForegroundColor Green
    } else {
        throw
    }
} catch {
    Write-Host "  ✗ Rust: NOT INSTALLED" -ForegroundColor Red
    Write-Host "    Run: .\install-rust.ps1" -ForegroundColor Yellow
    Write-Host "    Or download from: https://rustup.rs/" -ForegroundColor Yellow
    $allGood = $false
}

# Check Cargo
Write-Host "Checking Cargo..." -ForegroundColor Yellow
try {
    $cargoVersion = cargo --version 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ✓ Cargo: $cargoVersion" -ForegroundColor Green
    } else {
        throw
    }
} catch {
    Write-Host "  ✗ Cargo: NOT INSTALLED" -ForegroundColor Red
    $allGood = $false
}

# Check Ollama
Write-Host "Checking Ollama..." -ForegroundColor Yellow
try {
    $ollamaCheck = curl.exe -s http://localhost:11434/api/tags 2>&1
    if ($LASTEXITCODE -eq 0 -or $ollamaCheck -match "models") {
        Write-Host "  ✓ Ollama: Running" -ForegroundColor Green
    } else {
        throw
    }
} catch {
    Write-Host "  ⚠ Ollama: Not running or not installed" -ForegroundColor Yellow
    Write-Host "    Download from: https://ollama.ai/" -ForegroundColor Yellow
    Write-Host "    After installing, run: ollama pull llama3" -ForegroundColor Yellow
}

# Check npm dependencies
Write-Host "Checking npm dependencies..." -ForegroundColor Yellow
if (Test-Path "node_modules") {
    Write-Host "  ✓ npm dependencies: Installed" -ForegroundColor Green
} else {
    Write-Host "  ⚠ npm dependencies: Not installed" -ForegroundColor Yellow
    Write-Host "    Run: npm install" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan

if ($allGood) {
    Write-Host "✓ All core prerequisites are installed!" -ForegroundColor Green
    Write-Host ""
    Write-Host "You can now run:" -ForegroundColor Cyan
    Write-Host "  npm run tauri dev" -ForegroundColor White
} else {
    Write-Host "✗ Some prerequisites are missing." -ForegroundColor Red
    Write-Host ""
    Write-Host "Please install the missing tools and run this script again." -ForegroundColor Yellow
}

Write-Host ""
