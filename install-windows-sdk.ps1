# PowerShell script to install Windows SDK via Visual Studio Installer
# This script helps you install the Windows SDK component

Write-Host "Windows SDK Installation Guide" -ForegroundColor Cyan
Write-Host "==============================" -ForegroundColor Cyan
Write-Host ""

# Check if Visual Studio Installer is available
$vsInstallerPaths = @(
    "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vs_installer.exe",
    "${env:ProgramFiles}\Microsoft Visual Studio\Installer\vs_installer.exe"
)

$vsInstallerPath = $null
foreach ($path in $vsInstallerPaths) {
    if (Test-Path $path) {
        $vsInstallerPath = $path
        break
    }
}

if ($vsInstallerPath) {
    Write-Host "Visual Studio Installer found!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Opening Visual Studio Installer..." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Manual Steps:" -ForegroundColor Cyan
    Write-Host "1. Click 'Modify' on your Visual Studio Build Tools 2022 installation" -ForegroundColor White
    Write-Host "2. Ensure 'Desktop development with C++' workload is checked" -ForegroundColor White
    Write-Host "3. Under 'Individual components', search for 'Windows SDK'" -ForegroundColor White
    Write-Host "4. Check the latest 'Windows 10 SDK' or 'Windows 11 SDK' (e.g., 10.0.22621.0)" -ForegroundColor White
    Write-Host "5. Click 'Modify' to install" -ForegroundColor White
    Write-Host ""
    
    Start-Process $vsInstallerPath
    Write-Host "Visual Studio Installer opened. Follow the steps above." -ForegroundColor Green
} else {
    Write-Host "Visual Studio Installer not found!" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please install Visual Studio Build Tools first:" -ForegroundColor Yellow
    Write-Host "1. Download from: https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022" -ForegroundColor White
    Write-Host "2. Run the installer" -ForegroundColor White
    Write-Host "3. Select 'Desktop development with C++' workload" -ForegroundColor White
    Write-Host "4. Ensure 'Windows 10/11 SDK' is selected" -ForegroundColor White
    Write-Host "5. Install" -ForegroundColor White
}

Write-Host ""
Write-Host "Alternative: Install Windows SDK Standalone" -ForegroundColor Cyan
Write-Host "Download from: https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/" -ForegroundColor Yellow
