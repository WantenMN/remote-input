use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use remote_input::state::AppState;

const LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
const OTHER_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 99));

// ── Helpers ──────────────────────────────────────────────────

fn make_state(max_connections: usize, allow: Vec<IpAddr>) -> (AppState, tokio::sync::mpsc::UnboundedReceiver<String>) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let state = AppState {
        paste_tx: tx,
        connected: Arc::new(Mutex::new(HashMap::new())),
        max_connections,
        allow,
    };
    (state, rx)
}

async fn start_server(max_connections: usize, allow: Vec<IpAddr>) -> (SocketAddr, tokio::sync::mpsc::UnboundedReceiver<String>) {
    let (state, rx) = make_state(max_connections, allow);
    let addr = start_server_with_state(state).await;
    (addr, rx)
}

async fn start_server_with_state(state: AppState) -> SocketAddr {
    let app = remote_input::server::build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    addr
}

// ── CLI parsing ──────────────────────────────────────────────

mod cli {
    use std::net::IpAddr;
    use clap::Parser;
    use remote_input::cli::Args;

    #[test]
    fn defaults() {
        let args = Args::try_parse_from(["remote-input"]).unwrap();
        assert_eq!(args.port, 48732);
        assert_eq!(args.paste_delay, 20);
        assert!(!args.http);
        assert_eq!(args.max_connections, 1);
        assert!(args.allow.is_empty());
    }

    #[test]
    fn all_flags() {
        let args = Args::try_parse_from([
            "remote-input",
            "-p", "9999",
            "-D", "50",
            "--http",
            "-m", "5",
            "-a", "192.168.1.5,192.168.1.10",
        ])
        .unwrap();
        assert_eq!(args.port, 9999);
        assert_eq!(args.paste_delay, 50);
        assert!(args.http);
        assert_eq!(args.max_connections, 5);
        assert_eq!(args.allow, vec![
            "192.168.1.5".parse::<IpAddr>().unwrap(),
            "192.168.1.10".parse::<IpAddr>().unwrap(),
        ]);
    }

    #[test]
    fn short_flags() {
        let args = Args::try_parse_from(["remote-input", "-p", "8080", "-m", "10"]).unwrap();
        assert_eq!(args.port, 8080);
        assert_eq!(args.max_connections, 10);
    }

    #[test]
    fn invalid_port_fails() {
        assert!(Args::try_parse_from(["remote-input", "-p", "notanumber"]).is_err());
    }
}

// ── Access control ───────────────────────────────────────────

mod access {
    use remote_input::state::check_access;
    use super::*;

    #[test]
    fn empty_whitelist_allows_everyone() {
        let (state, _rx) = make_state(10, vec![]);
        assert!(check_access(&state, LOCALHOST).is_none());
        assert!(check_access(&state, OTHER_IP).is_none());
    }

    #[test]
    fn whitelist_allows_matching_ip() {
        let (state, _rx) = make_state(10, vec![LOCALHOST]);
        assert!(check_access(&state, LOCALHOST).is_none());
    }

    #[test]
    fn whitelist_rejects_non_matching_ip() {
        let (state, _rx) = make_state(10, vec![LOCALHOST]);
        let resp = check_access(&state, OTHER_IP);
        assert!(resp.is_some());
        assert_eq!(resp.unwrap().status(), axum::http::StatusCode::FORBIDDEN);
    }

    #[test]
    fn multiple_whitelist_entries() {
        let (state, _rx) = make_state(10, vec![LOCALHOST, OTHER_IP]);
        assert!(check_access(&state, LOCALHOST).is_none());
        assert!(check_access(&state, OTHER_IP).is_none());
        assert!(check_access(&state, "172.16.0.1".parse().unwrap()).is_some());
    }

    #[test]
    fn at_capacity_rejects_new_ip() {
        let (state, _rx) = make_state(1, vec![]);
        state.connected.lock().unwrap().insert(OTHER_IP, ());
        assert!(check_access(&state, LOCALHOST).is_some());
    }

    #[test]
    fn already_connected_ip_allowed_even_at_capacity() {
        let (state, _rx) = make_state(1, vec![]);
        state.connected.lock().unwrap().insert(LOCALHOST, ());
        assert!(check_access(&state, LOCALHOST).is_none());
    }

    #[test]
    fn under_capacity_allows() {
        let (state, _rx) = make_state(3, vec![]);
        state.connected.lock().unwrap().insert(OTHER_IP, ());
        assert!(check_access(&state, LOCALHOST).is_none());
    }

    #[test]
    fn whitelist_checked_before_capacity() {
        // Capacity=2, one slot used. A non-whitelisted IP should be rejected
        // by the whitelist check (not the capacity check).
        let (state, _rx) = make_state(2, vec![LOCALHOST]);
        state.connected.lock().unwrap().insert(LOCALHOST, ());
        let not_whitelisted: IpAddr = "10.0.0.88".parse().unwrap();
        let resp = check_access(&state, not_whitelisted).unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
    }
}

// ── HTTP routes ──────────────────────────────────────────────

mod http_routes {
    use super::*;

    #[tokio::test]
    async fn index_returns_200_html() {
        let (addr, _rx) = start_server(10, vec![]).await;
        let resp = reqwest::get(format!("http://{addr}/")).await.unwrap();
        assert_eq!(resp.status(), 200);
        assert!(resp.headers()["content-type"].to_str().unwrap().contains("text/html"));
    }

    #[tokio::test]
    async fn index_contains_root_div() {
        let (addr, _rx) = start_server(10, vec![]).await;
        let body = reqwest::get(format!("http://{addr}/"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(body.contains(r#"<div id="root">"#));
    }

    /// Find the first file matching an extension under web-dist/assets/.
    fn find_asset(ext: &str) -> String {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("web-dist/assets");
        std::fs::read_dir(&dir)
            .expect("web-dist/assets not found — run `cargo build` first")
            .flatten()
            .find(|e| e.path().extension().map_or(false, |e| e == ext))
            .map(|e| format!("/assets/{}", e.file_name().to_string_lossy()))
            .unwrap_or_else(|| panic!("no .{ext} asset found"))
    }

    #[tokio::test]
    async fn css_asset_returns_200() {
        let path = find_asset("css");
        let (addr, _rx) = start_server(10, vec![]).await;
        let resp = reqwest::get(format!("http://{addr}{path}")).await.unwrap();
        assert_eq!(resp.status(), 200);
        assert!(resp.headers()["content-type"].to_str().unwrap().contains("text/css"));
    }

    #[tokio::test]
    async fn js_asset_returns_200() {
        let path = find_asset("js");
        let (addr, _rx) = start_server(10, vec![]).await;
        let resp = reqwest::get(format!("http://{addr}{path}")).await.unwrap();
        assert_eq!(resp.status(), 200);
        assert!(resp.headers()["content-type"].to_str().unwrap().contains("javascript"));
    }

    #[tokio::test]
    async fn woff2_asset_returns_200() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("web-dist/assets");
        let entry = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .find(|e| e.path().extension().map_or(false, |e| e == "woff2"))
            .expect("no .woff2 asset found");
        let path = format!("/assets/{}", entry.file_name().to_string_lossy());

        let (addr, _rx) = start_server(10, vec![]).await;
        let resp = reqwest::get(format!("http://{addr}{path}")).await.unwrap();
        assert_eq!(resp.status(), 200);
        assert!(resp.headers()["content-type"].to_str().unwrap().contains("font"));
    }

    #[tokio::test]
    async fn unknown_asset_returns_404() {
        let (addr, _rx) = start_server(10, vec![]).await;
        let resp = reqwest::get(format!("http://{addr}/assets/nonexistent.txt"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test]
    async fn nonexistent_route_returns_404() {
        let (addr, _rx) = start_server(10, vec![]).await;
        let resp = reqwest::get(format!("http://{addr}/api/foo")).await.unwrap();
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test]
    async fn whitelist_rejects_non_whitelisted_ip() {
        // Server only allows OTHER_IP, but our request comes from 127.0.0.1.
        let (addr, _rx) = start_server(10, vec![OTHER_IP]).await;
        let resp = reqwest::get(format!("http://{addr}/")).await.unwrap();
        assert_eq!(resp.status(), 403);
        let body = resp.text().await.unwrap();
        assert!(body.contains("Access Denied"));
    }

    #[tokio::test]
    async fn fresh_server_allows_first_connection() {
        let (addr, _rx) = start_server(1, vec![]).await;
        let resp = reqwest::get(format!("http://{addr}/")).await.unwrap();
        assert_eq!(resp.status(), 200);
    }
}

// ── WebSocket ────────────────────────────────────────────────

mod websocket {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    /// Read messages until we get a Close frame, skipping Ping/Pong.
    /// Tolerates connection reset (server drops socket after sending close).
    async fn wait_for_close(
        ws: &mut (impl futures_util::Stream<Item = Result<Message, impl std::fmt::Debug>>
             + Unpin),
    ) -> Option<tokio_tungstenite::tungstenite::protocol::frame::CloseFrame> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match tokio::time::timeout_at(deadline, ws.next()).await {
                Ok(Some(Ok(Message::Close(frame)))) => return frame,
                Ok(Some(Ok(Message::Ping(_) | Message::Pong(_)))) => continue,
                // Server dropped connection after close — acceptable.
                Ok(Some(Err(_))) | Ok(None) | Err(_) => return None,
                _ => return None,
            }
        }
    }

    #[tokio::test]
    async fn connect_and_send_text() {
        let (addr, _rx) = start_server(10, vec![]).await;
        let (mut ws, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
                .await
                .unwrap();

        ws.send(Message::Text("hello world".into())).await.unwrap();

        let msg = ws.next().await.unwrap().unwrap();
        match msg {
            Message::Text(t) => {
                let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                assert_eq!(v["ok"], true);
                assert_eq!(v["len"], 11);
            }
            other => panic!("expected Text, got {other:?}"),
        }

        drop(ws);
    }

    #[tokio::test]
    async fn unicode_char_count() {
        let (addr, _rx) = start_server(10, vec![]).await;
        let (mut ws, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
                .await
                .unwrap();

        ws.send(Message::Text("你好".into())).await.unwrap();

        let msg = ws.next().await.unwrap().unwrap();
        match msg {
            Message::Text(t) => {
                let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                assert_eq!(v["ok"], true);
                assert_eq!(v["len"], 2);
            }
            other => panic!("expected Text, got {other:?}"),
        }

        drop(ws);
    }

    #[tokio::test]
    async fn whitespace_only_message_skipped() {
        let (addr, _rx) = start_server(10, vec![]).await;
        let (mut ws, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
                .await
                .unwrap();

        ws.send(Message::Text("   ".into())).await.unwrap();
        ws.send(Message::Text("real".into())).await.unwrap();

        // Should only get one ack (for "real").
        let msg = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            ws.next(),
        )
        .await
        .expect("timeout waiting for ack")
        .unwrap()
        .unwrap();
        match msg {
            Message::Text(t) => {
                let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                assert_eq!(v["ok"], true);
                assert_eq!(v["len"], 4);
            }
            other => panic!("expected Text ack, got {other:?}"),
        }

        drop(ws);
    }

    #[tokio::test]
    async fn text_is_trimmed() {
        let (addr, _rx) = start_server(10, vec![]).await;
        let (mut ws, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
                .await
                .unwrap();

        ws.send(Message::Text("  hello  ".into())).await.unwrap();

        let msg = ws.next().await.unwrap().unwrap();
        match msg {
            Message::Text(t) => {
                let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                assert_eq!(v["len"], 5); // "hello" after trim
            }
            other => panic!("expected Text, got {other:?}"),
        }

        drop(ws);
    }

    #[tokio::test]
    async fn whitelist_rejects_ws_with_close_frame() {
        let (addr, _rx) = start_server(10, vec![OTHER_IP]).await;
        let (mut ws, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
                .await
                .unwrap();

        let frame = wait_for_close(&mut ws).await.expect("no close frame received");
        assert_eq!(u16::from(frame.code), 1013);
        assert!(frame.reason.contains("not in the whitelist"));
    }

    #[tokio::test]
    async fn capacity_rejects_ws_with_close_frame() {
        // Pre-fill the connected set with a different IP so our localhost
        // connection gets rejected by the capacity check.
        let (state, _rx) = make_state(1, vec![]);
        state.connected.lock().unwrap().insert(OTHER_IP, ());
        let addr = start_server_with_state(state).await;

        let (mut ws, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
                .await
                .unwrap();

        let frame = wait_for_close(&mut ws).await.expect("no close frame received");
        assert_eq!(u16::from(frame.code), 1013);
        assert!(frame.reason.contains("capacity"));
    }

    #[tokio::test]
    async fn multiple_messages_in_sequence() {
        let (addr, _rx) = start_server(10, vec![]).await;
        let (mut ws, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
                .await
                .unwrap();

        for i in 0..5 {
            let text = format!("msg{i}");
            ws.send(Message::Text(text.clone().into())).await.unwrap();
            let msg = ws.next().await.unwrap().unwrap();
            match msg {
                Message::Text(t) => {
                    let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                    assert_eq!(v["ok"], true);
                    assert_eq!(v["len"], text.chars().count() as u64);
                }
                other => panic!("expected Text ack, got {other:?}"),
            }
        }

        drop(ws);
    }

    #[tokio::test]
    async fn paste_tx_receives_text() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState {
            paste_tx: tx,
            connected: Arc::new(Mutex::new(HashMap::new())),
            max_connections: 10,
            allow: vec![],
        };
        let addr = start_server_with_state(state).await;

        let (mut ws, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
                .await
                .unwrap();

        ws.send(Message::Text("paste me".into())).await.unwrap();

        let received = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout waiting for paste_tx")
            .expect("channel closed");
        assert_eq!(received, "paste me");

        drop(ws);
    }
}

// ── Network detection ────────────────────────────────────────

mod net {
    use remote_input::net::detect_lan_ip;

    #[test]
    fn returns_ipv4() {
        assert!(detect_lan_ip().is_ipv4());
    }

    #[test]
    fn returns_valid_ip() {
        let ip = detect_lan_ip();
        let _ = ip.to_string(); // must not panic
    }
}

// ── TLS certificate ──────────────────────────────────────────

mod tls {
    use remote_input::tls::ensure_cert;
    use std::net::IpAddr;

    #[tokio::test]
    async fn generates_cert_for_ip() {
        let ip: IpAddr = "192.168.99.99".parse().unwrap();
        let result = ensure_cert(ip).await;
        assert!(result.is_ok(), "ensure_cert failed: {:?}", result.err());
    }
}

// ── Linux group check ────────────────────────────────────────

#[cfg(target_os = "linux")]
mod linux_check {
    use remote_input::paste::linux_check::check_input_group;

    #[test]
    fn check_succeeds_on_dev_machine() {
        let result = check_input_group();
        assert!(result.is_ok(), "check_input_group failed: {}", result.unwrap_err());
    }
}
