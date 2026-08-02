use std::net::{IpAddr, SocketAddr};

use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use tracing::{error, info, warn};

use crate::state::AppState;

pub async fn handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
    let ip = addr.ip();

    // Whitelist check.
    if !state.allow.is_empty() && !state.allow.contains(&ip) {
        warn!("Rejected WebSocket from {ip}: not in whitelist");
        return ws.on_upgrade(move |mut socket| async move {
            let reason = format!("your IP ({ip}) is not in the whitelist");
            let _ = socket
                .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                    code: 1013,
                    reason: reason.into(),
                })))
                .await;
        });
    }

    // Capacity check before upgrading.
    {
        let mut conns = state.connected.lock().unwrap();
        if !conns.contains_key(&ip) && conns.len() >= state.max_connections {
            warn!(
                "Rejected connection from {ip}: limit reached ({}/{})",
                conns.len(),
                state.max_connections
            );
            return ws.on_upgrade(move |mut socket| async move {
                let _ = socket
                    .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                        code: 1013,
                        reason: "server is at capacity".into(),
                    })))
                    .await;
            });
        }
        conns.insert(ip, ());
    }

    info!("Client connected from {ip}");
    ws.on_upgrade(move |socket| handle_socket(socket, state, ip))
}

async fn handle_socket(mut socket: WebSocket, state: AppState, ip: IpAddr) {
    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            Message::Text(text) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let action = if cfg!(target_os = "windows") { "typing" } else { "pasting" };
                info!("Received {} chars from {ip}, {action}...", trimmed.chars().count());

                if let Err(e) = state.paste_tx.send(trimmed.to_string()) {
                    error!("Failed to send to paste worker: {e}");
                    break;
                }

                let ack = format!(r#"{{"ok":true,"len":{}}}"#, trimmed.chars().count());
                if socket.send(Message::Text(ack.into())).await.is_err() {
                    break;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    if let Ok(mut conns) = state.connected.lock() {
        conns.remove(&ip);
    }
    info!("Client {ip} disconnected");
}
