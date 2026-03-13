# ech0

![ech0](assets/ech0-glitch.gif)

[![GPL-3.0 License](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/tauri-2.10-blue.svg)](https://tauri.app)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://rust-lang.org)
[![I2P](https://img.shields.io/badge/i2p-embedded-blueviolet.svg)](https://geti2p.net)

Ephemeral P2P encrypted messaging over I2P. No server, no accounts, no message persistence.

---

## Properties

- **End-to-end encrypted** â€” X3DH key agreement + Double Ratchet (ChaCha20-Poly1305)
- **Anonymous transport** â€” all traffic routes through the embedded I2P router; no IPs exchanged
- **No persistence** â€” messages live only in RAM, wiped by TTL or on demand
- **Self-contained** â€” embedded I2P router, no external software required
- **No accounts** â€” identity is ephemeral, generated fresh each session

---

## How it works

On launch, ech0 starts an embedded I2P router and establishes an anonymous session. Once ready, a shareable `ech0://` link is generated. Send it to your peer over any channel â€” when they paste it in, both sides perform an X3DH handshake over the I2P tunnel and begin exchanging encrypted messages.

Messages expire automatically by a configurable TTL (30s / 1m / 5m / session). The wipe button destroys all messages, session keys, and identity material immediately.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full cryptographic and transport design.

---

## Releases

Pre-built binaries are published automatically via GitHub Actions on every version tag.
Download the latest from the [Releases](../../releases) page:

| Platform | Artifact |
|---|---|
| Windows | `ech0_*_x64-setup.exe` (NSIS installer) |
| macOS | `ech0_*.dmg` |
| Linux | `ech0_*_amd64.deb` / `.AppImage` |
| Android | `app-universal-release-unsigned.apk` |

---

## Building locally

### Common requirements (all platforms)

- [Node.js 20+](https://nodejs.org)
- [Rust via rustup](https://rustup.rs) â€” must use `rustup`, not a standalone installer

> **Dependency note:** `emissary-util` (the embedded I2P reseeder) requires a local patch for
> compatibility with Rust 1.77+. The patch lives in `vendor/emissary-util/` and is wired up
> via `[patch.crates-io]` in `src-tauri/Cargo.toml` â€” **no extra steps needed**, Cargo picks
> it up automatically.

---

### Desktop â€” Windows

**Additional requirements:**
- [Visual Studio Build Tools 2022](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with **Desktop development with C++**
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (included in Windows 11; install manually on Windows 10)

```powershell
.\scripts\build-windows.ps1
```

Output: `src-tauri\target\release\bundle\nsis\` and `\msi\`

---

### Desktop â€” macOS

**Additional requirements:**
- Xcode Command Line Tools: `xcode-select --install`

```bash
chmod +x scripts/build.sh && ./scripts/build.sh
```

Output: `src-tauri/target/release/bundle/dmg/`

---

### Desktop â€” Linux

**Additional requirements:**
```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libssl-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

```bash
chmod +x scripts/build.sh && ./scripts/build.sh
```

Output: `src-tauri/target/release/bundle/deb/` and `/appimage/`

---

### Android

The Android build toolchain requires Linux. On Windows, use **WSL2** (included with Windows 11; install via Microsoft Store on Windows 10).

#### Windows + WSL2 (one-liner setup)

```powershell
# First time: WSL2 + Ubuntu 24.04 setup (run once)
wsl --install -d Ubuntu-24.04

# Then, every build:
.\scripts\build-android.ps1
.\scripts\build-android.ps1 -Debug   # debug APK
```

The PowerShell script automatically converts line endings and delegates to WSL. The bash script inside WSL installs all dependencies on first run (Node 20, JDK 17, Android SDK, NDK 27.2.12479018, Rust Android targets).

#### macOS / Linux / WSL bash shell

```bash
chmod +x scripts/build-android.sh
bash scripts/build-android.sh           # release APK
bash scripts/build-android.sh --debug   # debug APK
```

The script installs all Android prerequisites automatically. Only `sudo` password required for apt packages.

Output: `src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk`

---

### Development mode

```bash
npm install
npm run tauri dev   # Vite + Tauri hot reload
```

---

## License

GPL-3.0. See [LICENSE](LICENSE).
