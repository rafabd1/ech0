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

Prerequisites: Rust, Node.js, Visual Studio Build Tools (Windows).

```sh
npm install
npm run tauri dev      # development build with hot reload
npm run tauri build    # release installer
```

Release output: `src-tauri/target/release/bundle/`

---

## License

GPL-3.0. See [LICENSE](LICENSE).