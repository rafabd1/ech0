# ech0

Ephemeral P2P encrypted messaging over I2P. No server, no accounts, no message history.

---

## Properties

- **End-to-end encrypted** — X3DH key agreement + symmetric Double Ratchet (ChaCha20-Poly1305)
- **Anonymous transport** — messages route through the I2P network; no IP addresses are exchanged
- **No persistence** — messages live only in RAM; wiped by configurable TTL or on demand
- **Self-contained** — embedded I2P router, no external software required
- **No accounts** — identity is ephemeral, generated fresh each session

---

## How it works

On launch, ech0 starts an embedded I2P router and establishes an anonymous session. Once the router is ready, a shareable `ech0://` link is generated. Send this link to your peer over any channel. When they paste it into their instance, both sides perform an X3DH handshake over the I2P tunnel and begin exchanging encrypted messages.

Messages expire automatically based on a configurable TTL (30s, 1min, 5min, or session-only). The wipe button destroys all messages, session keys, and identity material immediately.

See [ARCHITECTURE.md](ARCHITECTURE.md) for a full technical description of the cryptographic protocol, transport layer, and security model.

---

## Building

### Desktop — Windows

**Prerequisites:**
- [Node.js 20+](https://nodejs.org)
- [Rust via rustup](https://rustup.rs) (must use `rustup`, not standalone installer)
- [Visual Studio Build Tools 2022](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with "Desktop development with C++"
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (bundled with Windows 11; install separately on Windows 10)

```powershell
.\scripts\build-windows.ps1
# or skip frontend rebuild:
.\scripts\build-windows.ps1 -SkipFrontend
```

Output: `src-tauri\target\release\bundle\`

---

### Desktop — macOS / Linux

**Prerequisites:**
- [Node.js 20+](https://nodejs.org)
- [Rust via rustup](https://rustup.rs)
- macOS: Xcode Command Line Tools (`xcode-select --install`)
- Linux: `libwebkit2gtk-4.1`, `libssl`, `libgtk-3` (see [Tauri prereqs](https://tauri.app/start/prerequisites/))

```bash
chmod +x scripts/build.sh
./scripts/build.sh
```

Output: `src-tauri/target/release/bundle/` (`.dmg` on macOS, `.deb`/`.rpm` on Linux)

---

### Android

**Prerequisites (all platforms):**
- [Node.js 20+](https://nodejs.org)
- [Rust via rustup](https://rustup.rs)
- [JDK 17](https://adoptium.net/) (`JAVA_HOME` must be set)
- [Android Studio](https://developer.android.com/studio) with:
  - Android SDK (API 34+)
  - Android NDK (install via SDK Manager > SDK Tools)
  - `ANDROID_HOME` set to the SDK path

**Windows:**
```powershell
.\scripts\build-android.ps1
# debug build:
.\scripts\build-android.ps1 -Debug
```

**macOS / Linux:**
```bash
./scripts/build.sh android
# debug build:
./scripts/build.sh android debug
```

Output: `src-tauri/gen/android/app/build/outputs/apk/`

> **Note:** The build scripts auto-detect SDK/NDK paths and install missing Rust Android targets automatically.
>
> **Windows users**: Android build requires Unix-like build tools (Perl, shell, etc.). We recommend using **WSL2** or macOS/Linux instead. For details, see [ANDROID_BUILD_WINDOWS.md](ANDROID_BUILD_WINDOWS.md).

---

### Development (any platform)

```bash
npm install
npm run tauri dev      # hot reload
```

---

## License

GPL-3.0. See [LICENSE](LICENSE).