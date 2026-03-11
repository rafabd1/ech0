use serde::Serialize;
use tokio::{
    io::WriteHalf,
    net::TcpStream,
    sync::Mutex,
};

use crate::core::{
    crypto::{DoubleRatchet, IdentityKeys},
    memory::SecureBuffer,
    transport::I2pSession,
};

// ── Message entry (in-memory, never serialized to disk) ──────────────────────

pub struct MessageEntry {
    pub id: String,
    pub content: SecureBuffer,
    pub is_mine: bool,
    pub timestamp: u64,
    /// Unix timestamp when this message expires. 0 = session-only (no TTL).
    pub expires_at: u64,
}

/// Serializable view sent to the frontend via IPC.
#[derive(Serialize, Clone)]
pub struct MessageView {
    pub id: String,
    pub content: String,
    pub is_mine: bool,
    pub timestamp: u64,
    pub expires_at: u64,
}

impl From<&MessageEntry> for MessageView {
    fn from(e: &MessageEntry) -> Self {
        Self {
            id: e.id.clone(),
            content: String::from_utf8_lossy(e.content.as_bytes()).to_string(),
            is_mine: e.is_mine,
            timestamp: e.timestamp,
            expires_at: e.expires_at,
        }
    }
}

// ── Active peer session ───────────────────────────────────────────────────────

pub struct ActiveSession {
    pub peer_dest: String,
    pub peer_ik_bytes: [u8; 32],
    pub ratchet: DoubleRatchet,
    /// Write half of the active I2P tunnel stream.
    pub stream_writer: WriteHalf<TcpStream>,
    pub started_at: u64,
}

// ── Settings ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppSettings {
    /// Message TTL in seconds. 0 = session-only.
    pub ttl_seconds: u64,
    pub sam_address: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            ttl_seconds: 300,
            sam_address: "127.0.0.1:7656".to_string(),
        }
    }
}

// ── App state (Tauri managed) ─────────────────────────────────────────────────

pub struct AppState {
    pub identity: Mutex<Option<IdentityKeys>>,
    pub session: Mutex<Option<ActiveSession>>,
    pub messages: Mutex<Vec<MessageEntry>>,
    pub settings: Mutex<AppSettings>,
    pub i2p: Mutex<Option<I2pSession>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            identity: Mutex::new(None),
            session: Mutex::new(None),
            messages: Mutex::new(Vec::new()),
            settings: Mutex::new(AppSettings::default()),
            i2p: Mutex::new(None),
        }
    }
}
