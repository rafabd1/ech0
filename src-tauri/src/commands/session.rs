use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{split, AsyncWriteExt};
use x25519_dalek::{PublicKey, StaticSecret};
use rand::rngs::OsRng;

use crate::{
    core::{
        crypto::{generate_qr_svg, x3dh_initiator, x3dh_responder, DoubleRatchet, IdentityKeys},
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
    pub qr_svg: String,
}

#[derive(Serialize, Deserialize)]
pub struct SettingsPayload {
    pub ttl_seconds: u64,
    pub sam_address: String,
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Generate a new identity keypair. Called once on app startup.
#[tauri::command]
pub async fn generate_identity(state: State<'_, AppState>) -> Result<IdentityInfo, String> {
    let keys = IdentityKeys::generate();

    let qr_data = serde_json::json!({
        "dest": "",
        "k": keys.ik_pub_hex(),
        "s": keys.spk_pub_hex(),
    })
    .to_string();

    let info = IdentityInfo {
        b32_addr: String::new(),
        ik_pub_hex: keys.ik_pub_hex(),
        spk_pub_hex: keys.spk_pub_hex(),
        qr_svg: generate_qr_svg(&qr_data),
    };

    *state.identity.lock().await = Some(keys);
    Ok(info)
}

/// Connect to the I2P network via SAMv3 bridge.
/// Emits `identity_updated` with the real I2P address and regenerated QR.
#[tauri::command]
pub async fn connect_i2p(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let sam_addr = state.settings.lock().await.sam_address.clone();

    let session = I2pSession::connect(&sam_addr)
        .await
        .map_err(|e| e.to_string())?;

    let b32 = session.b32_addr.clone();
    let dest = session.destination.clone();
    let session_id = session.session_id.clone();
    let sam_clone = sam_addr.clone();

    {
        let id_lock = state.identity.lock().await;
        if let Some(keys) = id_lock.as_ref() {
            let qr_data = serde_json::json!({
                "dest": dest,
                "k": keys.ik_pub_hex(),
                "s": keys.spk_pub_hex(),
            })
            .to_string();

            let qr_svg = generate_qr_svg(&qr_data);
            let _ = app.emit(
                "identity_updated",
                serde_json::json!({ "b32_addr": b32, "qr_svg": qr_svg }),
            );
        }
    }

    *state.i2p.lock().await = Some(session);

    // Start background accept loop
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        accept_loop(app_clone, session_id, sam_clone).await;
    });

    Ok(())
}

/// Accept loop: continuously listens for incoming I2P connections.
async fn accept_loop(app: AppHandle, session_id: String, sam_addr: String) {

    loop {
        // Reconstruct a minimal session reference for accept
        let accept_result = {
            let state = app.state::<AppState>();
            let i2p = state.i2p.lock().await;
if let Some(_session) = i2p.as_ref() {
                drop(i2p);
                // We can't call accept_once while holding the lock due to borrowing,
                // so we use the session_id and sam_addr directly
                accept_once_raw(&session_id, &sam_addr).await
            } else {
                break;
            }
        };

        match accept_result {
            Ok((peer_dest, tunnel)) => {
                if let Err(e) = handle_incoming(&app, peer_dest, tunnel).await {
                    log::warn!("incoming session error: {}", e);
                }
            }
            Err(e) => {
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
        stream.read_exact(&mut byte).await?;
        if byte[0] == b'\n' { break; }
        if buf.len() > 16_384 { return Err(anyhow::anyhow!("line too long")); }
        buf.push(byte[0]);
    }
    Ok(String::from_utf8(buf)?)
}

/// Handle an incoming I2P connection: read handshake, compute X3DH, set session.
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

    // Read HANDSHAKE_INIT
    let frame = read_framed(&mut reader).await?;
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

    let root_key = {
        let id = state.identity.lock().await;
        let keys = id.as_ref().ok_or_else(|| anyhow::anyhow!("no identity"))?;
        x3dh_responder(&keys.ik_secret, &keys.spk_secret, &ik_a_pub, &ek_a_pub)
    };

    let ratchet = DoubleRatchet::from_root_key(&root_key, false);

    // Send HANDSHAKE_ACK
    let ack = serde_json::to_vec(&HandshakeAck { t: "ack".into() })?;
    write_framed(&mut writer, &ack).await?;

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

/// Initiate a session by scanning/pasting peer's QR payload.
#[tauri::command]
pub async fn initiate_session(
    state: State<'_, AppState>,
    app: AppHandle,
    peer_payload: String,
) -> Result<(), String> {
    #[derive(Deserialize)]
    struct PeerInfo { dest: String, k: String, s: String }

    let peer: PeerInfo = serde_json::from_str(&peer_payload)
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

    let root_key = {
        let id = state.identity.lock().await;
        let keys = id.as_ref().ok_or("no identity generated")?;
        x3dh_initiator(&keys.ik_secret, &ek_a, &ik_b_pub, &spk_b_pub)
    };

    let ratchet = DoubleRatchet::from_root_key(&root_key, true);

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

    // Send HANDSHAKE_INIT
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

    write_framed(&mut writer, &init_msg)
        .await
        .map_err(|e| e.to_string())?;

    // Wait for ACK
    let ack_frame = read_framed(&mut reader)
        .await
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
                    log::warn!("message handling error: {}", e);
                }
            }
            Err(e) => {
                log::info!("peer stream closed: {}", e);
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

    let content = String::from_utf8(plaintext_buf.as_bytes().to_vec())?;
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

/// Immediately zeroize all session and message state.
#[tauri::command]
pub async fn panic_wipe(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    // Drop session (zeroizes DoubleRatchet chain keys via SecureBuffer)
    {
        let mut sess = state.session.lock().await;
        if let Some(mut s) = sess.take() {
            let _ = s.stream_writer.shutdown().await;
        }
    }
    // Zeroize all messages
    state.messages.lock().await.clear();
    // Drop identity key material
    *state.identity.lock().await = None;
    // Close I2P session
    *state.i2p.lock().await = None;

    let _ = app.emit("panic_wipe", ());
    Ok(())
}

/// Update app settings.
#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: SettingsPayload,
) -> Result<(), String> {
    let mut s = state.settings.lock().await;
    s.ttl_seconds = settings.ttl_seconds;
    s.sam_address = settings.sam_address;
    Ok(())
}

/// Get current settings.
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsPayload, String> {
    let s = state.settings.lock().await;
    Ok(SettingsPayload {
        ttl_seconds: s.ttl_seconds,
        sam_address: s.sam_address.clone(),
    })
}
