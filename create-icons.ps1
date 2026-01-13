# Create placeholder icons for Tauri
# This script creates minimal valid icon files for development

$iconsDir = "src-tauri\icons"
New-Item -ItemType Directory -Force -Path $iconsDir | Out-Null

Write-Host "Creating placeholder icons..." -ForegroundColor Cyan

# Create a minimal 32x32 PNG (1x1 pixel, scaled)
$png32 = New-Object System.Drawing.Bitmap(32, 32)
$graphics = [System.Drawing.Graphics]::FromImage($png32)
$graphics.Clear([System.Drawing.Color]::FromArgb(74, 158, 255)) # #4a9eff
$graphics.Dispose()
$png32.Save("$iconsDir\32x32.png", [System.Drawing.Imaging.ImageFormat]::Png)
$png32.Dispose()

# Create 128x128 PNG
$png128 = New-Object System.Drawing.Bitmap(128, 128)
$graphics = [System.Drawing.Graphics]::FromImage($png128)
$graphics.Clear([System.Drawing.Color]::FromArgb(74, 158, 255))
$graphics.Dispose()
$png128.Save("$iconsDir\128x128.png", [System.Drawing.Imaging.ImageFormat]::Png)
$png128.Dispose()

# Create 256x256 PNG (128x128@2x)
$png256 = New-Object System.Drawing.Bitmap(256, 256)
$graphics = [System.Drawing.Graphics]::FromImage($png256)
$graphics.Clear([System.Drawing.Color]::FromArgb(74, 158, 255))
$graphics.Dispose()
$png256.Save("$iconsDir\128x128@2x.png", [System.Drawing.Imaging.ImageFormat]::Png)
$png256.Dispose()

# Create icon.png for system tray
$iconPng = New-Object System.Drawing.Bitmap(64, 64)
$graphics = [System.Drawing.Graphics]::FromImage($iconPng)
$graphics.Clear([System.Drawing.Color]::FromArgb(74, 158, 255))
$graphics.Dispose()
$iconPng.Save("$iconsDir\icon.png", [System.Drawing.Imaging.ImageFormat]::Png)
$iconPng.Dispose()

# Create ICO file (Windows)
$ico = New-Object System.Drawing.Icon([System.Drawing.SystemIcons]::Application, 32, 32)
$ico.ToBitmap().Save("$iconsDir\icon.ico", [System.Drawing.Imaging.ImageFormat]::Icon)
$ico.Dispose()

# For macOS, create a minimal ICNS (we'll skip this for now as it requires additional tools)
# The build should work without it on Windows

Write-Host "Placeholder icons created!" -ForegroundColor Green
Write-Host "Location: $iconsDir" -ForegroundColor Gray
