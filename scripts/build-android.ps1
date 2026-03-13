# ech0 — Android APK build (Windows wrapper)
# Delegates everything to scripts/build-android.sh running inside WSL2 Ubuntu-24.04
# First run installs all dependencies automatically inside WSL (prompts for sudo password)
# Usage: ./scripts/build-android.ps1 [-Debug]

param([switch]$Debug)

$root = Split-Path $PSScriptRoot -Parent
$wslPath = ($root -replace '\\', '/') -replace '^([A-Za-z]):', { "/mnt/$($args[0][0].ToString().ToLower())" }
$debugArg = if ($Debug) { "--debug" } else { "" }

# Ensure LF line endings in the bash script (Windows may add CRLF)
wsl -d Ubuntu-24.04 bash -c "sed -i 's/\r//' '$wslPath/scripts/build-android.sh'"

wsl -d Ubuntu-24.04 bash "$wslPath/scripts/build-android.sh" $debugArg
if ($LASTEXITCODE -ne 0) { Write-Error "Android build failed"; exit 1 }
