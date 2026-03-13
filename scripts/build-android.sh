#!/usr/bin/env bash
# ech0 — Android APK build (Linux / WSL2)
# Usage:
#   bash scripts/build-android.sh [--debug]
#
# First run installs all dependencies automatically.
# Requires sudo for apt packages (you will be prompted once).

set -euo pipefail

DEBUG_FLAG=""
for arg in "$@"; do
    [[ "$arg" == "--debug" ]] && DEBUG_FLAG="--debug"
done

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ANDROID_HOME="${ANDROID_HOME:-$HOME/android-sdk}"
NDK_VERSION="27.2.12479018"
BUILD_TOOLS_VERSION="35.0.0"
PLATFORM_VERSION="android-35"
CMDTOOLS_URL="https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip"

export ANDROID_HOME
export NDK_HOME="$ANDROID_HOME/ndk/$NDK_VERSION"
export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-17-openjdk-amd64}"
export PATH="$HOME/.cargo/bin:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"

log() { echo "==> $*"; }

# ── System packages ────────────────────────────────────────────────────────────

setup_packages() {
    local pkgs=()

    # Node 20+
    if ! command -v node &>/dev/null || [[ "$(node -e 'process.stdout.write(process.version)' 2>/dev/null)" < "v20" ]]; then
        log "Installing Node.js 20..."
        curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash - 2>/dev/null
        pkgs+=(nodejs)
    fi

    command -v java  &>/dev/null || pkgs+=(openjdk-17-jdk-headless)
    command -v unzip &>/dev/null || pkgs+=(unzip)
    command -v wget  &>/dev/null || pkgs+=(wget)
    command -v cc    &>/dev/null || pkgs+=(build-essential)
    command -v perl  &>/dev/null || pkgs+=(perl)
    dpkg -s libssl-dev &>/dev/null 2>&1 || pkgs+=(libssl-dev)

    if [[ ${#pkgs[@]} -gt 0 ]]; then
        log "Installing: ${pkgs[*]}"
        sudo apt-get install -y "${pkgs[@]}"
    fi
}

# ── Rust + Android targets ─────────────────────────────────────────────────────

setup_rust() {
    if ! command -v rustup &>/dev/null; then
        log "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi

    local installed
    installed=$(rustup target list --installed 2>/dev/null)
    for t in aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android; do
        echo "$installed" | grep -q "^$t" || rustup target add "$t"
    done
}

# ── Android SDK ────────────────────────────────────────────────────────────────

setup_android_sdk() {
    if [[ ! -f "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" ]]; then
        log "Installing Android SDK command-line tools..."
        mkdir -p "$ANDROID_HOME/cmdline-tools"
        wget -q "$CMDTOOLS_URL" -O /tmp/cmdtools.zip
        unzip -q /tmp/cmdtools.zip -d /tmp/cmdtools-tmp
        mv /tmp/cmdtools-tmp/cmdline-tools "$ANDROID_HOME/cmdline-tools/latest"
        rm -rf /tmp/cmdtools.zip /tmp/cmdtools-tmp
    fi

    export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$PATH"

    local installed_ndk="$ANDROID_HOME/ndk/$NDK_VERSION"
    if [[ ! -d "$installed_ndk" ]]; then
        log "Installing SDK components (NDK, platform, build-tools)..."
        yes | sdkmanager --licenses >/dev/null 2>&1 || true
        sdkmanager \
            "platform-tools" \
            "platforms;$PLATFORM_VERSION" \
            "build-tools;$BUILD_TOOLS_VERSION" \
            "ndk;$NDK_VERSION" 2>&1 | grep -Ev "^\[" || true
    fi
}

# ── NDK toolchain wrappers ────────────────────────────────────────────────────
# Modern NDK uses llvm-ranlib/llvm-ar but openssl-src expects aarch64-linux-android-ranlib etc.

setup_ndk_wrappers() {
    local NDK_BIN="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
    local WRAPPER_DIR="/tmp/android-ndk-wrappers"
    mkdir -p "$WRAPPER_DIR"

    for triple in aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android; do
        ln -sf "$NDK_BIN/llvm-ranlib" "$WRAPPER_DIR/${triple}-ranlib" 2>/dev/null || true
        ln -sf "$NDK_BIN/llvm-ar"     "$WRAPPER_DIR/${triple}-ar"     2>/dev/null || true
        ln -sf "$NDK_BIN/llvm-strip"  "$WRAPPER_DIR/${triple}-strip"  2>/dev/null || true
    done

    export PATH="$WRAPPER_DIR:$NDK_BIN:$PATH"
    log "NDK toolchain wrappers ready at $WRAPPER_DIR"
}

# ── APK signing ───────────────────────────────────────────────────────────────
# Signs the APK so it can be installed on real devices without developer mode.
# Release signing: set ANDROID_KEYSTORE_PATH + ANDROID_KEYSTORE_PASS +
#                  ANDROID_KEY_ALIAS + ANDROID_KEY_PASS in the environment.
# Debug signing:   a keystore is auto-generated at scripts/debug.keystore.

sign_apk() {
    local UNSIGNED_APK="$1"
    local SIGNED_APK="${UNSIGNED_APK/unsigned/signed}"
    local APKSIGNER="$ANDROID_HOME/build-tools/$BUILD_TOOLS_VERSION/apksigner"

    if [[ ! -f "$APKSIGNER" ]]; then
        log "apksigner not found at $APKSIGNER — skipping signing"
        return 0
    fi

    if [[ -n "${ANDROID_KEYSTORE_PATH:-}" ]]; then
        # ── Release keystore (from environment) ───────────────────────────────
        log "Signing APK with release keystore..."
        "$APKSIGNER" sign \
            --ks "$ANDROID_KEYSTORE_PATH" \
            --ks-key-alias "${ANDROID_KEY_ALIAS:-key0}" \
            --ks-pass "pass:${ANDROID_KEYSTORE_PASS}" \
            --key-pass "pass:${ANDROID_KEY_PASS:-$ANDROID_KEYSTORE_PASS}" \
            --out "$SIGNED_APK" \
            "$UNSIGNED_APK"
    else
        # ── Debug keystore (auto-generated for local testing) ─────────────────
        local KEYSTORE="$ROOT/scripts/debug.keystore"
        if [[ ! -f "$KEYSTORE" ]]; then
            log "Generating debug keystore at $KEYSTORE..."
            keytool -genkey -v \
                -keystore "$KEYSTORE" \
                -alias key0 \
                -keyalg RSA \
                -keysize 2048 \
                -validity 10000 \
                -storepass android \
                -keypass android \
                -dname "CN=ech0 Debug, O=ech0, C=US" \
                2>/dev/null
        fi
        log "Signing APK with debug keystore..."
        "$APKSIGNER" sign \
            --ks "$KEYSTORE" \
            --ks-key-alias key0 \
            --ks-pass pass:android \
            --key-pass pass:android \
            --out "$SIGNED_APK" \
            "$UNSIGNED_APK"
    fi

    log "Signed APK: $SIGNED_APK"
    echo "$SIGNED_APK"
}

# ── Build ──────────────────────────────────────────────────────────────────────

build() {
    log "Environment:"
    log "  ANDROID_HOME : $ANDROID_HOME"
    log "  NDK_HOME     : $NDK_HOME"
    log "  JAVA_HOME    : $JAVA_HOME"
    log "  Node.js      : $(node -v 2>/dev/null)"
    log "  Rust         : $(rustc --version 2>/dev/null)"

    cd "$ROOT"

    log "Installing Node dependencies (Linux)..."
    npm install 2>/dev/null

    if [[ ! -d "src-tauri/gen/android/app" ]]; then
        log "Initializing Tauri Android project..."
        npm run tauri android init
    fi

    log "Building frontend..."
    npm run build

    if [[ -n "$DEBUG_FLAG" ]]; then
        log "Building Android APK (debug)..."
        npm run tauri android build -- --debug
    else
        log "Building Android APK (release)..."
        npm run tauri android build
    fi

    echo ""
    APK=$(find "$ROOT/src-tauri/gen/android/app/build/outputs/apk" -name "*unsigned*.apk" 2>/dev/null | head -1)
    if [[ -z "$APK" ]]; then
        APK=$(find "$ROOT/src-tauri/gen/android/app/build/outputs/apk" -name "*.apk" 2>/dev/null | head -1)
    fi

    if [[ -n "$APK" && -z "$DEBUG_FLAG" ]]; then
        SIGNED=$(sign_apk "$APK")
        [[ -n "$SIGNED" ]] && log "Final APK: $SIGNED" || log "APK (unsigned): $APK"
    else
        [[ -n "$APK" ]] && log "APK: $APK" || log "Build complete (APK path not found, check output above)"
    fi
}

setup_packages
setup_rust
setup_android_sdk
setup_ndk_wrappers
build
