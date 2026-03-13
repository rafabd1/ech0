# Android Build via WSL2 — Complete Setup Guide

## Prerequisites
- Windows 11/10 with WSL2 enabled
- Ubuntu-24.04 distro installed: `wsl --install Ubuntu-24.04`
- Android SDK installed on Windows
- JDK 17 installed on Windows

---

## Step 1: Install Dependencies in WSL2

Run these commands **once** in PowerShell:

```powershell
wsl -d Ubuntu-24.04 sudo apt-get update -y
wsl -d Ubuntu-24.04 sudo apt-get install -y build-essential curl wget nodejs npm openjdk-17-jdk openjdk-17-jre
```

---

## Step 2: Install Rust & Android Targets

```powershell
wsl -d Ubuntu-24.04 bash -c 'curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
wsl -d Ubuntu-24.04 bash -c 'source ~/.cargo/env && rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android'
```

---

## Step 3: Set Up Android SDK in WSL

The easiest way is to create symlinks from Windows SDK to WSL:

```powershell
# In PowerShell
$androidHome = "$env:LOCALAPPDATA\Android\Sdk"
Write-Host "Android SDK path: $androidHome"

# Then in WSL Ubuntu shell:
wsl -d Ubuntu-24.04 bash -c 'mkdir -p /root/android-sdk && ln -sf /mnt/c/Users/rafae/AppData/Local/Android/Sdk/* /root/android-sdk/ 2>/dev/null || true'
```

---

## Step 4: Build APK

Now you can build:

```powershell
Set-Location "C:\Users\rafae\Desktop\projetos\ech0"
./scripts/build-android.ps1
```

Or with debug build:

```powershell
./scripts/build-android.ps1 -Debug
```

---

## Troubleshooting

**"Android SDK not found"**
- Verify Windows Android SDK location exists
- Check ANDROID_HOME in Windows: `$env:LOCALAPPDATA\Android\Sdk`
- Verify symlink in WSL: `wsl -d Ubuntu-24.04 ls /root/android-sdk`

**"Gradle build failed"**
- Ensure NDK is installed: Android Studio > SDK Manager > SDK Tools > NDK
- Check NDK version: `ls $env:LOCALAPPDATA\Android\Sdk\ndk\`

**"Node version mismatch"**
- WSL has Node 18, but some packages need Node 20+
- Install Node 20+ in WSL: `wsl -d Ubuntu-24.04 bash -c 'curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash - && sudo apt-get install -y nodejs'`

---

## Output Location

APK will be at:
- `src-tauri/gen/android/app/build/outputs/apk/*/release/*.apk`
