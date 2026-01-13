# Refresh PATH environment variable in current session
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

Write-Host "PATH refreshed!" -ForegroundColor Green
Write-Host ""

# Verify Rust is now available
if (Get-Command rustc -ErrorAction SilentlyContinue) {
    Write-Host "✓ Rust is now available!" -ForegroundColor Green
    rustc --version
    cargo --version
} else {
    Write-Host "✗ Rust still not found. Please close and reopen your terminal." -ForegroundColor Red
}
