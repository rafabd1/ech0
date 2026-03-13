#!/usr/bin/env bash
# ech0 — Build script for macOS and Linux
# Usage:
#   ./scripts/build.sh             # Windows/macOS/Linux desktop release
#   ./scripts/build.sh android     # Android APK
#   ./scripts/build.sh android debug

set -euo pipefail

TARGET="${1:-desktop}"
MODE="${2:-release}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

cd "$ROOT"

echo "==> ech0 build — target=$TARGET mode=$MODE"

# ── Helpers ──────────────────────────────────────────────────────────────────

require() {
    if ! command -v "$1" &>/dev/null; then
        echo "ERROR: '$1' not found. $2"
        exit 1
    fi
}

# ── Prerequisites check ───────────────────────────────────────────────────────

require node  "Install Node.js from https://nodejs.org"
require cargo "Install Rust from https://rustup.rs"
require rustup "Install Rust from https://rustup.rs"

if [[ "$TARGET" == "android" ]]; then
    require java "Install JDK 17: brew install --cask temurin17  (macOS) or apt install openjdk-17-jdk (Linux)"

    # Auto-detect Android SDK
    if [[ -z "${ANDROID_HOME:-}" ]]; then
        if [[ -d "$HOME/Library/Android/sdk" ]]; then
            export ANDROID_HOME="$HOME/Library/Android/sdk"
        elif [[ -d "$HOME/Android/Sdk" ]]; then
            export ANDROID_HOME="$HOME/Android/Sdk"
        else
            echo "ERROR: ANDROID_HOME not set. Install Android Studio and set ANDROID_HOME."
            exit 1
        fi
        echo "    ANDROID_HOME auto-detected: $ANDROID_HOME"
    fi

    # Auto-detect NDK (highest version)
    if [[ -z "${NDK_HOME:-}" ]]; then
        NDK_DIR="$ANDROID_HOME/ndk"
        if [[ -d "$NDK_DIR" ]]; then
            NDK_HOME="$(ls -1 "$NDK_DIR" | sort -rV | head -1)"
            NDK_HOME="$NDK_DIR/$NDK_HOME"
            export NDK_HOME
            echo "    NDK_HOME auto-detected: $NDK_HOME"
        else
            echo "ERROR: No NDK found in $NDK_DIR. Install NDK via Android Studio > SDK Manager."
            exit 1
        fi
    fi

    # Ensure Android Rust targets
    TARGETS=("aarch64-linux-android" "armv7-linux-androideabi" "x86_64-linux-android")
    INSTALLED=$(rustup target list --installed)
    for t in "${TARGETS[@]}"; do
        if ! echo "$INSTALLED" | grep -q "$t"; then
            echo "    Installing Rust target: $t"
            rustup target add "$t"
        fi
    done

    echo "    JAVA_HOME    : ${JAVA_HOME:-not set}"
    echo "    ANDROID_HOME : $ANDROID_HOME"
    echo "    NDK_HOME     : $NDK_HOME"
fi

# ── Frontend ──────────────────────────────────────────────────────────────────

echo "==> Building frontend"
npm install --prefer-offline 2>/dev/null || npm install
npm run build

# ── Tauri build ───────────────────────────────────────────────────────────────

if [[ "$TARGET" == "android" ]]; then
    # Init Android project if not done
    if [[ ! -d "$ROOT/src-tauri/gen/android/app" ]]; then
        echo "==> Initializing Tauri Android project (first time)"
        npm run tauri android init
    fi

    echo "==> Generating platform icons"
    npx tauri icon src-tauri/icons/icon.png

    if [[ "$MODE" == "debug" ]]; then
        echo "==> Building Android APK (debug)"
        npm run tauri android build -- --debug
    else
        echo "==> Building Android APK (release)"
        npm run tauri android build
    fi

    APK=$(find "$ROOT/src-tauri/gen/android/app/build/outputs/apk" -name "*.apk" 2>/dev/null | head -1)
    echo ""
    echo "==> Build complete"
    [[ -n "$APK" ]] && echo "    APK: $APK"
else
    echo "==> Building Tauri release"
    npm run tauri build

    echo ""
    echo "==> Build complete"
    # macOS
    APP=$(find "$ROOT/src-tauri/target/release/bundle" -name "*.dmg" 2>/dev/null | head -1)
    [[ -n "$APP" ]] && echo "    DMG: $APP"
    # Linux
    DEB=$(find "$ROOT/src-tauri/target/release/bundle" -name "*.deb" 2>/dev/null | head -1)
    [[ -n "$DEB" ]] && echo "    DEB: $DEB"
    RPM=$(find "$ROOT/src-tauri/target/release/bundle" -name "*.rpm" 2>/dev/null | head -1)
    [[ -n "$RPM" ]] && echo "    RPM: $RPM"
fi
