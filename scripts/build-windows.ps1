# ech0 — Windows release build
# Usage: ./scripts/build-windows.ps1
# Output: src-tauri/target/release/bundle/nsis/ech0_*_x64-setup.exe

param(
    [switch]$SkipFrontend
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent

Set-Location $root

Write-Host "==> ech0 Windows build"

# Ensure Rust is in PATH (standalone WinGet install location)
$rustBin = "C:\Program Files\Rust stable MSVC 1.93\bin"
if ((Test-Path $rustBin) -and ($env:PATH -notlike "*$rustBin*")) {
    $env:PATH = "$rustBin;$env:PATH"
    Write-Host "    Added Rust to PATH: $rustBin"
}

# Build frontend
if (-not $SkipFrontend) {
    Write-Host "==> Building frontend (npm run build)"
    npm run build
    if ($LASTEXITCODE -ne 0) { Write-Error "Frontend build failed"; exit 1 }
}

# Build Tauri release
Write-Host "==> Building Tauri release (npm run tauri build)"
npm run tauri build
if ($LASTEXITCODE -ne 0) { Write-Error "Tauri build failed"; exit 1 }

# Locate output
$nsis = Get-ChildItem "$root\src-tauri\target\release\bundle\nsis\*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
$msi  = Get-ChildItem "$root\src-tauri\target\release\bundle\msi\*.msi"  -ErrorAction SilentlyContinue | Select-Object -First 1
$exe  = "$root\src-tauri\target\release\ech0.exe"

Write-Host ""
Write-Host "==> Build complete"
if ($nsis) { Write-Host "    Installer : $($nsis.FullName)" }
if ($msi)  { Write-Host "    MSI       : $($msi.FullName)" }
if (Test-Path $exe) { Write-Host "    Binary    : $exe" }
