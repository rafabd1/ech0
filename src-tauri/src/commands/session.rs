use std::time::{SystemTime, UNIX_EPOCH};

use base64::{
    engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD},
    Engine as _,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{split, AsyncWriteExt};
use tokio::time::{timeout, Duration};
use x25519_dalek::{PublicKey, StaticSecret};
use rand::rngs::OsRng;
use zeroize::Zeroize;

use crate::{
    core::{
        crypto::{x3dh_initiator, x3dh_responder, DoubleRatchet, IdentityKeys},
        memory::SecureBuffer,
        transport::{read_framed, write_framed, I2pSession},
    },
    state::{ActiveSession, AppState, MessageEntry, MessageView},
};

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct HandshakeInit {
    t: String,
    ik: String,
    ek: String,
}

#[derive(Serialize, Deserialize)]
struct HandshakeAck {
    t: String,
}

#[derive(Serialize)]
pub struct IdentityInfo {
    pub b32_addr: String,
    pub ik_pub_hex: String,
    pub spk_pub_hex: String,
    /// Shareable ech0:// link (empty until I2P session is established).
    pub connect_link: String,
}

#[derive(Serialize, Deserialize)]
pub struct SettingsPayload {
    pub ttl_seconds: u64,
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Return the current identity, or generate a fresh one if none exists.
/// This ensures the frontend never overwrites keys that the SAM session was built with.
#[tauri::command]
pub async fn generate_identity(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<IdentityInfo, String> {
    let mut id_lock = state.identity.lock().await;

    if let Some(keys) = id_lock.as_ref() {
        // Identity already set — return current info (SAM may already be connected)
        let connect_link = {
            let i2p = state.i2p.lock().await;
            if let Some(session) = i2p.as_ref() {
                build_connect_link(&session.destination, &keys.ik_pub_hex(), &keys.spk_pub_hex())
            } else {
                String::new()
            }
        };
        let b32 = {
            let i2p = state.i2p.lock().await;
            i2p.as_ref().map(|s| s.b32_addr.clone()).unwrap_or_default()
        };
        return Ok(IdentityInfo {
            b32_addr: b32,
            ik_pub_hex: keys.ik_pub_hex(),
            spk_pub_hex: keys.spk_pub_hex(),
            connect_link,
        });
    }

    // No identity yet (first run or after wipe) — generate a new one
    let keys = IdentityKeys::generate();
    let info = IdentityInfo {
        b32_addr: String::new(),
        ik_pub_hex: keys.ik_pub_hex(),
        spk_pub_hex: keys.spk_pub_hex(),
        connect_link: String::new(),
    };
    *id_lock = Some(keys);
    drop(id_lock);

    // If SAM session becomes ready before frontend calls this again, the
    // identity_updated event will carry the correct connect_link.
    let _ = app;
    Ok(info)
}

/// Connect to the embedded I2P router via SAMv3 (manual trigger).
#[tauri::command]
pub async fn connect_i2p(app: AppHandle) -> Result<(), String> {
    do_connect_i2p(app).await.map_err(|e| e.to_string())
}

/// Internal: connect to embedded SAM, build I2P session, emit events.
pub async fn do_connect_i2p(app: AppHandle) -> anyhow::Result<()> {
    let state = app.state::<AppState>();

    let sam_port = state
        .router_sam_port
        .lock()
        .await
        .ok_or_else(|| anyhow::anyhow!("embedded router not ready"))?;

    let sam_addr = format!("127.0.0.1:{}", sam_port);

    let session = tokio::time::timeout(
        tokio::time::Duration::from_secs(120),
        I2pSession::connect(&sam_addr),
    )
    .await
    .map_err(|_| anyhow::anyhow!("SAM connection timed out"))?
    .map_err(|e| anyhow::anyhow!("SAM connect: {}", e))?;

    let b32 = session.b32_addr.clone();
    let dest = session.destination.clone();
    let session_id = session.session_id.clone();
    let sam_clone = sam_addr.clone();

    let connect_link = {
        let id_lock = state.identity.lock().await;
        let keys = id_lock
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no identity -- call generate_identity first"))?;
        build_connect_link(&dest, &keys.ik_pub_hex(), &keys.spk_pub_hex())
    };

    let _ = app.emit(
        "identity_updated",
        serde_json::json!({ "b32_addr": b32, "connect_link": connect_link }),
    );

    *state.i2p.lock().await = Some(session);
    *state.router_status.lock().await = "ready".to_string();
    let _ = app.emit("router_status_changed", "ready");

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        accept_loop(app_clone, session_id, sam_clone).await;
    });

    Ok(())
}

/// Background retry loop: keeps trying to connect to SAM until it succeeds.
pub async fn auto_connect_loop(app: AppHandle) {
    loop {
        {
            let state = app.state::<AppState>();
            if state.i2p.lock().await.is_some() {
                return;
            }
        }
        match do_connect_i2p(app.clone()).await {
            Ok(()) => return,
            Err(e) => {
                #[cfg(debug_assertions)]
                log::debug!("SAM connect attempt: {}, retrying in 5s", e);
                let _ = e;
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }
}

/// Build an ech0:// shareable link from I2P destination and identity keys.
fn build_connect_link(dest: &str, ik_hex: &str, spk_hex: &str) -> String {
    let json = serde_json::json!({ "dest": dest, "k": ik_hex, "s": spk_hex }).to_string();
    format!("ech0://{}", URL_SAFE_NO_PAD.encode(json.as_bytes()))
}

/// Accept loop: waits for incoming I2P stream connections for this session.
async fn accept_loop(app: AppHandle, session_id: String, sam_addr: String) {
    loop {
        // Exit if the session was replaced or dropped (e.g. after panic_wipe)
        let should_continue = {
            let state = app.state::<AppState>();
            let i2p = state.i2p.lock().await;
            i2p.as_ref().map_or(false, |s| s.session_id == session_id)
        };
        if !should_continue {
            break;
        }

        match accept_once_raw(&session_id, &sam_addr).await {
            Ok((peer_dest, tunnel)) => {
                if let Err(e) = handle_incoming(&app, peer_dest, tunnel).await {
                    // Emit user-visible error on handshake failure
                    let error_msg = format!("Connection failed: {}", e);
                    let _ = app.emit("connection_error", error_msg);
                    
                    #[cfg(debug_assertions)]
                    log::warn!("incoming session error: {}", e);
                    let _ = e;
                }
            }
            Err(e) => {
                #[cfg(debug_assertions)]
                log::warn!("STREAM ACCEPT failed: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        }
    }
}

async fn accept_once_raw(
    session_id: &str,
    sam_addr: &str,
) -> anyhow::Result<(String, tokio::net::TcpStream)> {
    let mut stream = tokio::net::TcpStream::connect(sam_addr).await?;

    stream.write_all(b"HELLO VERSION MIN=3.1 MAX=3.3\n").await?;
    let hello_reply = read_sam_line_raw(&mut stream).await?;
    if !hello_reply.contains("RESULT=OK") {
        return Err(anyhow::anyhow!("SAM HELLO failed"));
    }

    let cmd = format!("STREAM ACCEPT ID={} SILENT=false\n", session_id);
    stream.write_all(cmd.as_bytes()).await?;

    let status = read_sam_line_raw(&mut stream).await?;
    if !status.contains("RESULT=OK") {
        return Err(anyhow::anyhow!("STREAM ACCEPT failed: {}", status));
    }

    let peer_dest = read_sam_line_raw(&mut stream).await?;
    Ok((peer_dest.trim().to_string(), stream))
}

async fn read_sam_line_raw(stream: &mut tokio::net::TcpStream) -> anyhow::Result<String> {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::with_capacity(512);
    let mut byte = [0u8; 1];
    loop {
        stream.read_exact(&mut byte[..]).await?;
        if byte[0] == b'\n' { break; }
        if buf.len() > 16_384 { return Err(anyhow::anyhow!("line too long")); }
        buf.push(byte[0]);
    }
    Ok(String::from_utf8(buf)?)
}

/// Handle an incoming I2P connection: read handshake, compute X3DH, set session.
/// All I/O operations are wrapped in timeouts to prevent indefinite hangs on degraded I2P tunnels.
async fn handle_incoming(
    app: &AppHandle,
    peer_dest: String,
    tunnel: tokio::net::TcpStream,
) -> anyhow::Result<()> {
    let state = app.state::<AppState>();

    // Reject if session already active
    if state.session.lock().await.is_some() {
        return Ok(());
    }

    let (mut reader, mut writer) = split(tunnel);

    // Read HANDSHAKE_INIT with timeout (60 seconds given I2P latency)
    let frame = timeout(Duration::from_secs(60), read_framed(&mut reader))
        .await
        .map_err(|_| anyhow::anyhow!("handshake timeout: peer did not send INIT within 60s"))?;
    
    let init: HandshakeInit = serde_json::from_slice(&frame)?;
    if init.t != "hi" {
        return Err(anyhow::anyhow!("expected handshake init, got type={}", init.t));
    }

    let ik_a_bytes = hex::decode(&init.ik)?;
    let ek_a_bytes = hex::decode(&init.ek)?;
    if ik_a_bytes.len() != 32 || ek_a_bytes.len() != 32 {
        return Err(anyhow::anyhow!("invalid key lengths in handshake"));
    }

    let ik_a_pub = PublicKey::from(<[u8; 32]>::try_from(ik_a_bytes.as_slice())?);
    let ek_a_pub = PublicKey::from(<[u8; 32]>::try_from(ek_a_bytes.as_slice())?);

    let mut root_key = {
        let id = state.identity.lock().await;
        let keys = id.as_ref().ok_or_else(|| anyhow::anyhow!("no identity"))?;
        x3dh_responder(&keys.ik_secret, &keys.spk_secret, &ik_a_pub, &ek_a_pub)
    };

    let ratchet = DoubleRatchet::from_root_key(&root_key, false);
    root_key.zeroize();

    // Send HANDSHAKE_ACK with timeout (30 seconds - writing should be fast)
    let ack = serde_json::to_vec(&HandshakeAck { t: "ack".into() })?;
    timeout(Duration::from_secs(30), write_framed(&mut writer, &ack))
        .await
        .map_err(|_| anyhow::anyhow!("handshake timeout: failed to send ACK within 30s"))?;

    let peer_ik_bytes = <[u8; 32]>::try_from(ik_a_bytes.as_slice())?;

    *state.session.lock().await = Some(ActiveSession {
        peer_dest: peer_dest.clone(),
        peer_ik_bytes,
        ratchet,
        stream_writer: writer,
        started_at: now_secs(),
    });

    // Emit session established event
    let _ = app.emit("session_established", serde_json::json!({ "peer_dest": peer_dest }));

    // Spawn receive loop
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        receive_loop(app_clone, reader).await;
    });

    Ok(())
}

/// Initiate a session by pasting peer's ech0:// link or raw JSON payload.
#[tauri::command]
pub async fn initiate_session(
    state: State<'_, AppState>,
    app: AppHandle,
    peer_payload: String,
) -> Result<(), String> {
    #[derive(Deserialize)]
    struct PeerInfo { dest: String, k: String, s: String }

    // Accept both ech0:// links and raw JSON
    let json_str = if peer_payload.trim_start().starts_with("ech0://") {
        let encoded = peer_payload.trim_start().trim_start_matches("ech0://");
        let decoded = URL_SAFE_NO_PAD
            .decode(encoded.trim())
            .map_err(|e| format!("invalid ech0 link: {}", e))?;
        String::from_utf8(decoded)
            .map_err(|e| format!("invalid ech0 link encoding: {}", e))?
    } else {
        peer_payload.clone()
    };

    let peer: PeerInfo = serde_json::from_str(&json_str)
        .map_err(|e| format!("invalid peer payload: {}", e))?;

    if peer.dest.is_empty() || peer.k.is_empty() || peer.s.is_empty() {
        return Err("peer payload missing required fields".into());
    }

    let ik_b_bytes = hex::decode(&peer.k).map_err(|e| e.to_string())?;
    let spk_b_bytes = hex::decode(&peer.s).map_err(|e| e.to_string())?;
    if ik_b_bytes.len() != 32 || spk_b_bytes.len() != 32 {
        return Err("invalid key lengths in peer info".into());
    }

    let ik_b_pub = PublicKey::from(<[u8; 32]>::try_from(ik_b_bytes.as_slice()).unwrap());
    let spk_b_pub = PublicKey::from(<[u8; 32]>::try_from(spk_b_bytes.as_slice()).unwrap());

    // Generate ephemeral key and compute X3DH
    let ek_a = StaticSecret::random_from_rng(OsRng);
    let ek_a_pub = PublicKey::from(&ek_a);

    let mut root_key = {
        let id = state.identity.lock().await;
        let keys = id.as_ref().ok_or("no identity generated")?;
        x3dh_initiator(&keys.ik_secret, &ek_a, &ik_b_pub, &spk_b_pub)
    };

    let ratchet = DoubleRatchet::from_root_key(&root_key, true);
    root_key.zeroize();

    // Dial peer
    let tunnel = {
        let i2p = state.i2p.lock().await;
        let session = i2p.as_ref().ok_or("i2p not connected")?;
        session
            .connect_to_peer(&peer.dest)
            .await
            .map_err(|e| e.to_string())?
    };

    let (mut reader, mut writer) = split(tunnel);

    // Send HANDSHAKE_INIT with timeout (30 seconds - writing should be fast)
    let ik_hex = {
        let id = state.identity.lock().await;
        id.as_ref().unwrap().ik_pub_hex()
    };

    let init_msg = serde_json::to_vec(&HandshakeInit {
        t: "hi".into(),
        ik: ik_hex,
        ek: hex::encode(ek_a_pub.as_bytes()),
    })
    .map_err(|e| e.to_string())?;

    timeout(Duration::from_secs(30), write_framed(&mut writer, &init_msg))
        .await
        .map_err(|_| "handshake timeout: failed to send INIT within 30s")?
        .map_err(|e| e.to_string())?;

    // Wait for ACK with timeout (60 seconds given I2P latency)
    let ack_frame = timeout(Duration::from_secs(60), read_framed(&mut reader))
        .await
        .map_err(|_| "handshake timeout: peer did not send ACK within 60s")?
        .map_err(|e| e.to_string())?;
    
    let ack: HandshakeAck = serde_json::from_slice(&ack_frame).map_err(|e| e.to_string())?;
    if ack.t != "ack" {
        return Err(format!("unexpected ack type: {}", ack.t));
    }

    let peer_dest = peer.dest.clone();

    *state.session.lock().await = Some(ActiveSession {
        peer_dest: peer_dest.clone(),
        peer_ik_bytes: <[u8; 32]>::try_from(ik_b_bytes.as_slice()).unwrap(),
        ratchet,
        stream_writer: writer,
        started_at: now_secs(),
    });

    let _ = app.emit("session_established", serde_json::json!({ "peer_dest": peer_dest }));

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        receive_loop(app_clone, reader).await;
    });

    Ok(())
}

/// Background task: receive encrypted messages from the peer.
async fn receive_loop(app: AppHandle, mut reader: tokio::io::ReadHalf<tokio::net::TcpStream>) {
    loop {
        match read_framed(&mut reader).await {
            Ok(frame) => {
                if let Err(e) = handle_incoming_message(&app, &frame).await {
                    #[cfg(debug_assertions)]
                    log::warn!("message handling error: {}", e);
                    let _ = e;
                }
            }
            Err(e) => {
                #[cfg(debug_assertions)]
                log::info!("peer stream closed: {}", e);
                let _ = &e;
                let state = app.state::<AppState>();
                *state.session.lock().await = None;
                let _ = app.emit("session_closed", ());
                break;
            }
        }
    }
}

#[derive(Deserialize)]
struct WireMessage {
    t: String,
    id: String,
    ct: String,
    n: u32,
}

async fn handle_incoming_message(app: &AppHandle, frame: &[u8]) -> anyhow::Result<()> {
    let wire: WireMessage = serde_json::from_slice(frame)?;
    if wire.t != "msg" {
        return Ok(());
    }

    let ct = B64
        .decode(&wire.ct)
        .map_err(|e| anyhow::anyhow!("base64 decode: {}", e))?;

    let state = app.state::<AppState>();
    let settings = state.settings.lock().await.clone();

    let plaintext_buf = {
        let mut sess = state.session.lock().await;
        let session = sess.as_mut().ok_or_else(|| anyhow::anyhow!("no session"))?;
        session.ratchet.decrypt(&ct, wire.n)?
    };

    let mut content = String::from_utf8(plaintext_buf.as_bytes().to_vec())?;
    let now = now_secs();
    let expires_at = if settings.ttl_seconds > 0 {
        now + settings.ttl_seconds
    } else {
        0
    };

    let entry = MessageEntry {
        id: wire.id.clone(),
        content: SecureBuffer::from_slice(content.as_bytes()),
        is_mine: false,
        timestamp: now,
        expires_at,
    };
    // Wipe plaintext intermediate — the content now lives only in SecureBuffer
    unsafe { content.as_bytes_mut().zeroize(); }

    let view = MessageView::from(&entry);
    state.messages.lock().await.push(entry);
    let _ = app.emit("message_received", view);

    Ok(())
}

/// Close the active session, zeroizing all session key material.
#[tauri::command]
pub async fn close_session(state: State<'_, AppState>) -> Result<(), String> {
    let mut sess = state.session.lock().await;
    if let Some(mut s) = sess.take() {
        // Shut down write half to signal peer
        let _ = s.stream_writer.shutdown().await;
    }
    state.messages.lock().await.clear();
    Ok(())
}

/// Core wipe logic — shared by the panic_wipe command and the Android lifecycle hook.
pub async fn do_panic_wipe(app: AppHandle) {
    use tauri::{Emitter, Manager};
    let state = app.state::<AppState>();

    // Capture before clearing: if the router never started, we must restart it.
    let router_is_running = state.router_sam_port.lock().await.is_some();

    {
        let mut sess = state.session.lock().await;
        if let Some(mut s) = sess.take() {
            let _ = s.stream_writer.shutdown().await;
        }
    }
    state.messages.lock().await.clear();
    *state.identity.lock().await = None;
    *state.i2p.lock().await = None;

    // Status hint: if router is up just reconnect SAM; if not, we need to bootstrap first.
    let initial_status = if router_is_running { "connecting" } else { "bootstrapping" };
    *state.router_status.lock().await = initial_status.to_string();
    let _ = app.emit("panic_wipe", ());
    let _ = app.emit("router_status_changed", initial_status);

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;

        if router_is_running {
            // Router is up — just create a fresh SAM session for a new I2P identity.
            auto_connect_loop(app_clone).await;
        } else {
            // Router never started (failed on startup). Retry it now so the user
            // can recover without restarting the app.
            let data_dir = app_clone
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from(".ech0_data"));

            let mut attempt = 0u32;
            loop {
                match crate::core::router::start_embedded_router(data_dir.clone()).await {
                    Ok(sam_port) => {
                        *app_clone.state::<AppState>().router_sam_port.lock().await = Some(sam_port);
                        crate::set_router_status(&app_clone, "connecting").await;
                        auto_connect_loop(app_clone).await;
                        break;
                    }
                    Err(e) => {
                        attempt += 1;
                        let _ = e;
                        if attempt >= 3 {
                            crate::set_router_status(&app_clone, "error").await;
                            break;
                        }
                        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                        crate::set_router_status(&app_clone, "bootstrapping").await;
                    }
                }
            }
        }
    });
}

/// Wipe all sensitive state: messages, session keys, identity, I2P session.
/// Spawns a new auto-connect so the app recovers with a fresh I2P identity.
#[tauri::command]
pub async fn panic_wipe(
    _state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    do_panic_wipe(app).await;
    Ok(())
}

/// Update app settings.
#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: SettingsPayload,
) -> Result<(), String> {
    state.settings.lock().await.ttl_seconds = settings.ttl_seconds;
    Ok(())
}

/// Get current app settings.
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsPayload, String> {
    let s = state.settings.lock().await;
    Ok(SettingsPayload { ttl_seconds: s.ttl_seconds })
}

/// Get current router status. Called by the frontend on mount to avoid
/// missing events that fired before the WebView registered its listeners.
#[tauri::command]
pub async fn get_router_status(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.router_status.lock().await.clone())
}

/// Return the session fingerprint for out-of-band identity verification.
/// Both peers must see the same value to confirm no MITM.
#[tauri::command]
pub async fn get_safety_numbers(state: State<'_, AppState>) -> Result<String, String> {
    let id = state.identity.lock().await;
    let keys = id.as_ref().ok_or("no identity")?;
    let sess = state.session.lock().await;
    let session = sess.as_ref().ok_or("no active session")?;
    Ok(crate::core::crypto::safety_numbers(
        keys.ik_public.as_bytes(),
        &session.peer_ik_bytes,
    ))
}