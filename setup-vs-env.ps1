# PowerShell script to set up Visual Studio environment variables
# This captures the environment from vcvarsall.bat and applies it to the current PowerShell session

$vcvarsPath = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat"

if (-not (Test-Path $vcvarsPath)) {
    Write-Host "ERROR: Visual Studio Build Tools not found!" -ForegroundColor Red
    exit 1
}

Write-Host "Setting up Visual Studio environment..." -ForegroundColor Cyan

# Run vcvarsall.bat and capture the environment
$tempFile = [System.IO.Path]::GetTempFileName()
cmd /c "`"$vcvarsPath`" x64 > `"$tempFile`" 2>&1 && set > `"$tempFile`""

# Read the environment variables
$envVars = Get-Content $tempFile | Where-Object { $_ -match '^[^=]+=.*' }

foreach ($line in $envVars) {
    if ($line -match '^([^=]+)=(.*)$') {
        $name = $matches[1]
        $value = $matches[2]
        [Environment]::SetEnvironmentVariable($name, $value, "Process")
    }
}

Remove-Item $tempFile -ErrorAction SilentlyContinue

Write-Host "Environment set up successfully!" -ForegroundColor Green
Write-Host ""
Write-Host "You can now run: npm run tauri dev" -ForegroundColor Yellow
