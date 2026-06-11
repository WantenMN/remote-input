use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use axum::response::{Html, IntoResponse};
use tokio::sync::mpsc;
use tracing::warn;

/// Tracks which client IPs are currently connected.
pub type ConnectedIps = Arc<Mutex<HashMap<IpAddr, ()>>>;

/// Shared application state passed to all handlers.
#[derive(Clone)]
pub struct AppState {
    pub paste_tx: mpsc::UnboundedSender<String>,
    pub connected: ConnectedIps,
    pub max_connections: usize,
    pub allow: Vec<IpAddr>,
}

/// Check whitelist and capacity. Returns `Some(rejection_page)` if denied.
pub fn check_access(state: &AppState, ip: IpAddr) -> Option<axum::response::Response> {
    if !state.allow.is_empty() && !state.allow.contains(&ip) {
        warn!("Rejected request from {ip}: not in whitelist");
        let body = format!(
            "<h2>Access Denied</h2>\
             <p>Your IP (<b>{ip}</b>) is not in the whitelist.</p>\
             <p>To allow it, restart with:</p>\
             <pre>remote-input -a {ip}</pre>\
             <p>For multiple IPs:</p>\
             <pre>remote-input -a {ip},&lt;other-ip&gt;</pre>",
        );
        return Some(reject_page(body));
    }

    {
        let conns = state.connected.lock().unwrap();
        if !conns.contains_key(&ip) && conns.len() >= state.max_connections {
            warn!("Rejected request from {ip}: limit reached");
            let body = format!(
                "<h2>Server Full</h2>\
                 <p>At capacity ({}/{} connections).</p>\
                 <p>To allow more, restart with:</p>\
                 <pre>remote-input -m &lt;number&gt;</pre>\
                 <p>For example:</p>\
                 <pre>remote-input -m 3</pre>",
                conns.len(),
                state.max_connections,
            );
            return Some(reject_page(body));
        }
    }

    None
}

fn reject_page(body: String) -> axum::response::Response {
    let html = format!(
        "<!DOCTYPE html>\
         <html>\
         <head>\
         <meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Remote Input - Rejected</title>\
         <style>\
           body {{ font-family: -apple-system, sans-serif; padding: 2em; max-width: 600px; margin: auto; color: #333; }}\
           h2 {{ color: #c00; }}\
           pre {{ background: #f4f4f4; padding: 0.8em; border-radius: 4px; overflow-x: auto; font-size: 0.95em; }}\
         </style>\
         </head>\
         <body>{}</body>\
         </html>",
        body,
    );
    (axum::http::StatusCode::FORBIDDEN, Html(html)).into_response()
}
