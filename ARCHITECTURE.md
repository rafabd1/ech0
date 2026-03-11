# ech0 — Architecture

## Overview

ech0 is a Tauri v2 application. The Rust backend handles all cryptography, networking, and memory-sensitive operations. The React frontend handles only presentation and user interaction; it holds no secrets and no sensitive state persists in the browser runtime.

---

## Startup Sequence

```
App launch
  │
  ├─ Rust setup (synchronous, before window opens)
  │   └─ Generate X25519 identity keypair → store in AppState
  │
  ├─ Spawn: TTL wiper (1s background loop)
  │
  └─ Spawn: I2P router task (async)
      ├─ emit router_status_changed("bootstrapping")
      ├─ start_embedded_router(app_data_dir)
      │   ├─ Load or reseed I2P router cache (~50+ routers required)
      │   ├─ Generate NTCP2 key/IV via OsRng
      │   └─ RouterBuilder<TokioRuntime>::new(config).build() → spawn
      ├─ Store SAM port in AppState.router_sam_port
      ├─ emit router_status_changed("connecting")
      └─ auto_connect_loop (retries every 5s until SAM accepts session)
          └─ do_connect_i2p()
              ├─ I2pSession::connect(sam_addr) → SESSION CREATE STYLE=STREAM
              ├─ Build ech0:// link from session + identity keys
              ├─ emit identity_updated(b32_addr, connect_link)
              ├─ Store I2pSession in AppState.i2p
              ├─ emit router_status_changed("ready")
              └─ Spawn: accept_loop (waits for incoming connections)
```

The identity keypair is generated before the router starts, ensuring `do_connect_i2p` always finds a valid identity when building the session link.

---

## Session Link Format

```
ech0://<base64url_nopad(JSON)>
```

Where JSON is:
```json
{"dest":"<i2p_destination_base64>","k":"<ik_pub_hex>","s":"<spk_pub_hex>"}
```

- `dest` — full I2P SAMv3 base64 destination (~512 bytes). Used for `STREAM CONNECT`. Opaque to the app; meaningful only to the I2P router.
- `k` — hex-encoded X25519 identity public key (32 bytes). Used as `IK_b_pub` in X3DH.
- `s` — hex-encoded X25519 signed prekey public key (32 bytes). Used as `SPK_b_pub` in X3DH.

The link is valid only for the current session. I2P destinations are TRANSIENT: a new one is created on every app start. There is no long-term addressing.

---

## Cryptographic Protocol

### Key Generation

Each session generates two X25519 keypairs:
- **IK (identity key)** — long-term for the session duration
- **SPK (signed prekey)** — used in X3DH as the static DH component

Both are held in `IdentityKeys` and never serialized to disk.

### Handshake — X3DH

The initiator (peer A) generates a fresh ephemeral key EK_a and computes:

```
DH1 = X25519(IK_a, SPK_b)
DH2 = X25519(EK_a, IK_b)
DH3 = X25519(EK_a, SPK_b)
root_key = HKDF-SHA256(salt=0xFF*32, ikm=DH1||DH2||DH3, info="ech0_x3dh_v1")
```

The wire message is `{"t":"hi","ik":"<ik_a_hex>","ek":"<ek_a_hex>"}`.

The responder (peer B) computes the same root key as:

```
DH1 = X25519(SPK_b, IK_a)
DH2 = X25519(IK_b, EK_a)
DH3 = X25519(SPK_b, EK_a)
root_key = HKDF-SHA256(same derivation)
```

Both sides arrive at the same `root_key` without transmitting it. The responder sends `{"t":"ack"}` to confirm.

### Message Encryption — Symmetric Double Ratchet

Each message advances the symmetric ratchet:

```
message_key  = HMAC-SHA256(chain_key, 0x01)
next_ck      = HMAC-SHA256(chain_key, 0x02)
ciphertext   = ChaCha20-Poly1305(key=message_key, nonce=[0x00*8 || counter_be32], plaintext)
```

The `counter` is included in the wire message as field `n` and used both as nonce input and for in-order delivery enforcement. The receiver rejects any message whose counter does not match the expected `recv_count`.

Current limitation: symmetric ratchet only. The DH ratchet step (providing post-compromise security / break-in recovery) is deferred to v2.

### Wire Framing

All messages over the I2P STREAM tunnel use length-prefix framing:

```
[u32 BE: payload_len][JSON payload bytes]
```

Maximum frame size: 512 KB (enforced on read).

---

## Transport Layer

### Embedded I2P Router

`core/router.rs` starts an `emissary-core` router in-process using the Tokio async runtime. Configuration:

- **NTCP2** transport with `port=0` (OS-assigned), `publish=false` (client-only, not reachable as a relay)
- **SAMv3** bridge on a dynamically chosen loopback port
- **No transit tunnels**, no floodfill participation
- **Router cache** — on first run, reseeds from public I2P reseed servers via HTTPS (uses `emissary-util::Reseeder`). Cached router infos are stored at `app_data_dir/i2p_router_cache.bin` in a simple length-prefixed binary format. Minimum 50 routers required to use cache; otherwise re-reseeds.

### SAMv3 Client

`core/transport.rs` implements a minimal SAMv3 TCP client:

- **SESSION CREATE** — creates a STREAM-style session with a TRANSIENT destination using `SIGNATURE_TYPE=EdDSA_SHA512_Ed25519`. The control socket is held open for the lifetime of `I2pSession`; closing it terminates the I2P session.
- **STREAM ACCEPT** — opens a separate TCP connection to SAM and waits for an inbound connection. Returns the peer's full I2P destination and the raw tunnel `TcpStream`.
- **STREAM CONNECT** — dials a peer destination through the I2P network. Returns the tunnel `TcpStream` on success.

The `accept_loop` runs as a background task. It checks the session ID on each iteration and exits if the session has been replaced (e.g., after panic wipe).

---

## Memory Model

### SecureBuffer

All sensitive byte sequences (message content, chain keys, root key, X3DH intermediates) are stored in `SecureBuffer`:

```rust
pub struct SecureBuffer(Vec<u8>);

impl Drop for SecureBuffer {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
```

On Unix, `mlock()` is called on construction to prevent pages from being swapped to disk (best-effort; failure is non-fatal and does not crash the app). On Windows/Android, `VirtualLock` is skipped; `zeroize` on drop is the primary guarantee.

No sensitive type implements `Serialize` or `Clone` in a way that would copy key material outside the controlled buffer.

### AppState

All mutable state is protected by `tokio::sync::Mutex`:

- `identity: Mutex<Option<IdentityKeys>>`
- `session: Mutex<Option<ActiveSession>>`
- `messages: Mutex<Vec<MessageEntry>>`
- `i2p: Mutex<Option<I2pSession>>`
- `router_sam_port: Mutex<Option<u16>>`

The frontend only receives sanitized views (`MessageView`, `IdentityInfo`) via Tauri IPC. Raw key material never crosses the IPC boundary.

---

## Security Analysis

### What is protected

**Message content** — lives only in `SecureBuffer` instances inside `Vec<MessageEntry>`. Wiped by TTL background task (every 1s) or immediately on `panic_wipe` / `close_session`.

**Session keys** — the `DoubleRatchet` struct and `ActiveSession.stream_writer` are dropped and zeroized when the session ends.

**Identity keys** — dropped on `panic_wipe`. Re-generated on next startup or after wipe when `generate_identity` is called.

**IP address / physical location** — traffic exits through I2P garlic routing. The peer receives only the I2P destination, never the IP. The embedded router uses `publish=false`, so the local node does not appear in the I2P netDB as a reachable router.

**Disk artifacts** — no SQLite, no Tauri cache, no message logs. Only the I2P router cache (`i2p_router_cache.bin`) is written, which contains public I2P router infos — no user data.

### Known Gaps

**No DH ratchet.** The current implementation is a symmetric ratchet. If an attacker captures the chain key at message N, they can decrypt all subsequent messages in that direction until the session ends. Full post-compromise security requires the DH ratchet step (v2).

**Strict in-order delivery.** The ratchet counter must match `recv_count` exactly. Out-of-order or dropped messages cause decryption failure and session state divergence. No message skipping/buffering is implemented.

**Log file.** `tauri-plugin-log` writes to a file in `app_data_dir` in release builds. This file may contain I2P peer destination IDs and session event metadata. The file target should be disabled for production releases.

**Router cache forensics.** `i2p_router_cache.bin` is observable on disk. It contains only public I2P infrastructure router infos (no user identity, no destinations, no messages), but its presence confirms I2P usage.

**I2P timing correlation.** A global passive adversary with visibility into I2P tunnel traffic can correlate entry and exit timing to de-anonymize sessions over time — identical limitation to Tor onion routing.

**Cold boot.** Message content in RAM is vulnerable if the device is physically seized during an active session before TTL fires or panic wipe is triggered. `mlock` mitigates swap risk on Unix; Windows has no equivalent guarantee.

**No peer identity pinning.** The `ech0://` link contains the peer's public keys, but there is no certificate or long-term identity to pin across sessions. If an attacker intercepts the link before the peer receives it, they could substitute their own keys. The security model assumes the link is transmitted over a pre-existing trusted channel (e.g., Signal, in-person).

**CSP disabled.** `tauri.conf.json` sets `"csp": null`. For a local-only WebView loading from Tauri's asset server this is low-risk, but enabling a strict CSP is recommended for production.
