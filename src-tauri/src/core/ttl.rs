use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};
use tokio::time::{interval, Duration};

use crate::state::AppState;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Background task that wipes expired messages every second.
/// Dropping a MessageEntry triggers SecureBuffer::drop → zeroize.
pub async fn ttl_wiper(app: tauri::AppHandle) {
    let mut tick = interval(Duration::from_secs(1));
    loop {
        tick.tick().await;

        let state = app.state::<AppState>();
        let mut msgs = state.messages.lock().await;
        let now = now_secs();
        let before = msgs.len();

        msgs.retain(|m| m.expires_at == 0 || m.expires_at > now);

        if msgs.len() != before {
            let views: Vec<_> = msgs
                .iter()
                .map(|m| crate::state::MessageView {
                    id: m.id.clone(),
                    content: String::from_utf8_lossy(m.content.as_bytes()).to_string(),
                    is_mine: m.is_mine,
                    timestamp: m.timestamp,
                    expires_at: m.expires_at,
                })
                .collect();
            drop(msgs);
            let _ = app.emit("messages_updated", views);
        }
    }
}
