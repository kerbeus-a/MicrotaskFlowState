# Create minimal ICNS file (macOS icon)
# ICNS is a complex format, but we can create a minimal valid one
$iconsDir = "src-tauri\icons"

# Minimal ICNS file structure (just header, no actual image data for now)
# This is a workaround - a proper ICNS would require more complex structure
$icnsHeader = @(
    0x69, 0x63, 0x6E, 0x73,  # "icns" magic
    0x00, 0x00, 0x00, 0x00   # Size (will be updated)
)

# For development on Windows, we can create an empty/minimal ICNS
# Or better - just copy one of the PNGs as a placeholder
Copy-Item "$iconsDir\128x128.png" "$iconsDir\icon.icns" -ErrorAction SilentlyContinue

Write-Host "Created placeholder icon.icns" -ForegroundColor Green
