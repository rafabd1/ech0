# ech0 — Android Build Setup

## Prerequisites Installation

### 1. JDK 17 (required for Android build tools)

```powershell
# Check if already installed
java -version

# If not, install via winget
winget install EclipseAdoptium.Temurin.17.JDK

# Verify
java -version
```

### 2. Android Studio (SDK, NDK, Build Tools)

```powershell
# Install Android Studio
winget install Google.AndroidStudio

# After installation, open Android Studio and:
# 1. Click "Tools" → "SDK Manager"
# 2. Install:
#    - Android SDK Platform 34 (API 34)
#    - NDK (Side by side): version 25.2.9519653 or 26.x
#    - Android SDK Build-Tools 34.x
#    - Android SDK Tools (latest)
# 3. Close Android Studio
```

### 3. Set environment variables

Add to your PowerShell profile (`$PROFILE`) or set globally:

```powershell
# Find your profile location
echo $PROFILE

# Edit it (create if doesn't exist)
notepad $PROFILE

# Add these lines:
$env:JAVA_HOME = "C:\Program Files\Eclipse Adoptium\jdk-17.0.x"  # Adjust version
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:NDK_HOME = "$env:ANDROID_HOME\ndk\26.1.10909125"  # Adjust NDK version

# Verify paths exist:
# - $env:ANDROID_HOME should contain `platforms/`, `build-tools/`, `ndk/`
# - $env:NDK_HOME should contain `toolchains/`, `platforms/`
```

Reload PowerShell after editing profile.

### 4. Rust Android targets

```powershell
rustup target add aarch64-linux-android      # arm64 (primary, most devices)
rustup target add armv7-linux-androideabi    # arm32 (older devices, optional)
rustup target add x86_64-linux-android       # x86_64 (emulator, optional)

# Verify
rustup target list | Select-String android
```

### 5. Verify all paths

```powershell
# Run these checks
java -version                                  # JDK 17
echo $env:ANDROID_HOME                         # Should show SDK path
echo $env:NDK_HOME                             # Should show NDK path
ls "$env:ANDROID_HOME\platforms\"               # Should list API levels (e.g., android-34)
ls "$env:NDK_HOME\toolchains\"                  # Should list toolchains
rustup target list | Select-String android     # Should show installed Android targets
```

---

## Build for Android

### First time setup

```powershell
cd C:\Users\rafae\Desktop\projetos\ech0

# Initialize Tauri Android project (once only)
npm run tauri android init
```

This creates `src-tauri/gen/android/` with the Android app manifest, build config, etc.

### Build APK

```powershell
cd C:\Users\rafae\Desktop\projetos\ech0

# Full release build (this will take 15-30 minutes on first run)
npm run tauri android build --release

# Or debug build (faster for testing)
npm run tauri android build
```

### Output location

- **Release APK**: `src-tauri/gen/android/app/build/outputs/apk/release/app-release.apk`
- **Debug APK**: `src-tauri/gen/android/app/build/outputs/apk/debug/app-debug.apk`

### Install on device or emulator

```powershell
# via adb (from Android SDK)
adb install -r "src-tauri/gen/android/app/build/outputs/apk/release/app-release.apk"

# Launch app
adb shell am start -n dev.ech0.app/.MainActivity
```

---

## Troubleshooting

### "NDK not found"
- Verify `$env:NDK_HOME` is set correctly
- Check that the path exists: `ls "$env:NDK_HOME\toolchains"`
- The NDK version in `$env:NDK_HOME` must match one you installed (check in Android Studio > SDK Manager > Android SDK > Android NDK)

### "JAVA_HOME not set"
- Verify `$env:JAVA_HOME` points to a JDK 17 installation
- Check: `$env:JAVA_HOME\bin\java.exe` exists
- Reload PowerShell after editing `$PROFILE`

### "emissary-core fails to compile for Android"
- `emissary-core` is pure Rust and should compile cleanly via NDK
- If it fails, check:
  - Rust targets are installed: `rustup target add aarch64-linux-android`
  - NDK version is recent (25.x or 26.x)
  - No offline build attempt (emissary needs to fetch dependencies)

### APK is very large (>200 MB)
- This is normal for a first build (includes Rust binary + I2P router)
- Release build with `--release` applies optimizations and stripping
- Future builds will be cached and faster

### App crashes on Android startup
- Check logs: `adb logcat | grep ech0`
- Ensure I2P router bootstrap isn't timing out (check `ARCHITECTURE.md` startup sequence)
- Bootstrap can take 60-120s on first run; don't kill the app prematurely

---

## Signing Release APK (for Google Play or distribution)

For production distribution, you need to sign the APK with a release key:

```powershell
# Create a keystore (one-time)
keytool -genkey -v -keystore ech0-release.keystore ^
  -keyalg RSA -keysize 4096 -validity 10000 ^
  -alias ech0-key

# Configure gradle to use the keystore
# Edit src-tauri/gen/android/app/build.gradle:
# In the android { signingConfigs { } } section, add:
#
#   release {
#       storeFile file("path/to/ech0-release.keystore")
#       storePassword "your_keystore_password"
#       keyAlias "ech0-key"
#       keyPassword "your_key_password"
#   }
#
# And in buildTypes { release { signingConfig signingConfigs.release } }

npm run tauri android build --release
```

The signed APK is ready for Google Play.
