use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::Serialize;
use tauri::State;

use crate::{
    core::{memory::SecureBuffer, transport::write_framed},
    state::{AppState, MessageEntry, MessageView},
};

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Serialize)]
struct WireMessage<'a> {
    t: &'a str,
    id: &'a str,
    ct: String,
    n: u32,
}

/// Encrypt and send a message to the active peer.
/// Returns the MessageView so the frontend can add it to the list immediately.
#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    content: String,
) -> Result<MessageView, String> {
    if content.is_empty() {
        return Err("message content cannot be empty".into());
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = now_secs();

    let (ct, counter) = {
        let mut sess = state.session.lock().await;
        let session = sess.as_mut().ok_or("no active session")?;
        session
            .ratchet
            .encrypt(content.as_bytes())
            .map_err(|e| e.to_string())?
    };

    let wire = serde_json::to_vec(&WireMessage {
        t: "msg",
        id: &id,
        ct: B64.encode(&ct),
        n: counter,
    })
    .map_err(|e| e.to_string())?;

    {
        let mut sess = state.session.lock().await;
        let session = sess.as_mut().ok_or("session dropped between locks")?;
        write_framed(&mut session.stream_writer, &wire)
            .await
            .map_err(|e| e.to_string())?;
    }

    let settings = state.settings.lock().await.clone();
    let expires_at = if settings.ttl_seconds > 0 {
        now + settings.ttl_seconds
    } else {
        0
    };

    let entry = MessageEntry {
        id: id.clone(),
        content: SecureBuffer::from_slice(content.as_bytes()),
        is_mine: true,
        timestamp: now,
        expires_at,
    };

    let view = MessageView::from(&entry);
    state.messages.lock().await.push(entry);

    Ok(view)
}

/// Return all currently held messages as views.
#[tauri::command]
pub async fn get_messages(state: State<'_, AppState>) -> Result<Vec<MessageView>, String> {
    let msgs = state.messages.lock().await;
    Ok(msgs.iter().map(MessageView::from).collect())
}
