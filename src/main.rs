use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as AxumPath, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rust_embed::Embed;
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use rcgen::{CertificateParams, KeyPair, SanType};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Detect the machine's LAN IP address, skipping TUN/VPN interfaces.
fn detect_lan_ip() -> IpAddr {
    // Interface name substrings that indicate TUN/VPN/tunnel adapters.
    const TUN_KEYWORDS: &[&str] = &[
        "tun", "tap", "wireguard", "tailscale", "vpn", "tunnel", "loopback",
    ];

    if let Ok(ifaces) = local_ip_address::list_afinet_netifas() {
        for (_name, ip) in &ifaces {
            // Only consider IPv4 addresses.
            let IpAddr::V4(v4) = *ip else { continue };
            // Skip loopback (127.x.x.x).
            if v4.is_loopback() { continue }
            // Skip link-local (169.254.x.x).
            if v4.is_link_local() { continue }
            // Skip well-known TUN/VPN ranges: 100.64.0.0/10 (CGNAT, used by
            // Tailscale), 172.16.0.0/12 (common VPN default), and 10.0.0.0/8.
            // We still want these if there's nothing else, so only skip them
            // when the *name* also matches a tunnel keyword.
            let name_lower = _name.to_lowercase();
            if TUN_KEYWORDS.iter().any(|kw| name_lower.contains(kw)) {
                continue;
            }
            return IpAddr::V4(v4);
        }
    }

    // Fallback: use the crate's default detection (connects a UDP socket to
    // 8.8.8.8 to find the outbound IP).
    local_ip_address::local_ip().unwrap_or(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))
}

/// Generate a self-signed TLS certificate for the given LAN IP, or load it
/// from disk if one was generated in a previous run.
async fn ensure_cert(local_ip: IpAddr) -> anyhow::Result<RustlsConfig> {
    let dir = std::env::current_exe()?
        .parent()
        .unwrap_or(Path::new("."))
        .join("remote-input-cert");
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");

    // Re-use existing cert if present.
    if cert_path.exists() && key_path.exists() {
        info!("Loading TLS cert from {}", dir.display());
        let cert_pem = std::fs::read(&cert_path)?;
        let key_pem = std::fs::read(&key_path)?;
        let config = RustlsConfig::from_pem(cert_pem, key_pem).await?;
        return Ok(config);
    }

    info!("Generating self-signed TLS certificate...");
    std::fs::create_dir_all(&dir)?;

    let key_pair = KeyPair::generate()?;
    let mut params = CertificateParams::new(vec![local_ip.to_string()])?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, rcgen::DnValue::Utf8String("remote-input".into()));
    params
        .subject_alt_names
        .push(SanType::IpAddress(local_ip));

    let cert = params.self_signed(&key_pair)?;
    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    std::fs::write(&cert_path, &cert_pem)?;
    std::fs::write(&key_path, &key_pem)?;

    let config = RustlsConfig::from_pem(cert_pem.into_bytes(), key_pem.into_bytes()).await?;
    Ok(config)
}

/// Remote Input Daemon - type on your phone, paste into your Linux desktop
#[derive(Parser, Debug)]
#[command(name = "remote-input", version, about)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value_t = 48732)]
    port: u16,

    /// Delay in milliseconds between clipboard write and paste simulation
    #[arg(short = 'D', long, default_value_t = 20)]
    paste_delay: u64,

    /// Use HTTP instead of HTTPS (insecure, not recommended)
    #[arg(long, default_value_t = false)]
    http: bool,

    /// Maximum number of distinct client IPs allowed at once
    #[arg(short = 'm', long, default_value_t = 1)]
    max_connections: usize,

    /// Only allow connections from these IPs (comma-separated, e.g. -a 192.168.1.5,192.168.1.10)
    #[arg(short = 'a', long = "allow", value_name = "IP", value_delimiter = ',')]
    allow: Vec<IpAddr>,
}

/// Tracks which client IPs are currently connected.
type ConnectedIps = Arc<Mutex<HashMap<IpAddr, ()>>>;

/// Shared application state
#[derive(Clone)]
struct AppState {
    paste_tx: mpsc::UnboundedSender<String>,
    connected: ConnectedIps,
    max_connections: usize,
    allow: Vec<IpAddr>,
}

#[derive(Embed)]
#[folder = "web-dist/"]
struct Frontend;

// ── Linux uinput helpers ──────────────────────────────────────

#[cfg(target_os = "linux")]
mod uinput {
    use evdev::{AttributeSet, InputEvent, KeyCode};
    use std::time::Duration;

    /// Simulate Ctrl+Shift+V via /dev/uinput (evdev crate).
    pub fn paste_keystroke() -> Result<(), String> {
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::KEY_LEFTCTRL);
        keys.insert(KeyCode::KEY_LEFTSHIFT);
        keys.insert(KeyCode::KEY_V);

        let mut dev = evdev::uinput::VirtualDevice::builder()
            .map_err(|e| format!("Cannot open /dev/uinput: {e}. Are you in the 'input' group?"))?
            .name("remote-input virtual keyboard")
            .with_keys(&keys)
            .map_err(|e| format!("Failed to set key capabilities: {e}"))?
            .build()
            .map_err(|e| format!("Cannot create uinput device: {e}. Are you in the 'input' group?"))?;

        // Give the kernel a moment to register the device.
        std::thread::sleep(Duration::from_millis(100));

        let events: Vec<InputEvent> = [
            (KeyCode::KEY_LEFTCTRL, 1),
            (KeyCode::KEY_LEFTSHIFT, 1),
            (KeyCode::KEY_V, 1),
            (KeyCode::KEY_V, 0),
            (KeyCode::KEY_LEFTSHIFT, 0),
            (KeyCode::KEY_LEFTCTRL, 0),
        ]
        .map(|(code, value)| InputEvent::new(evdev::EventType::KEY.0, code.0, value))
        .to_vec();

        dev.emit(&events)
            .map_err(|e| format!("Failed to emit key events: {e}"))?;

        // Wait briefly for the input system to process the events.
        std::thread::sleep(Duration::from_millis(10));

        // VirtualDevice is dropped here, destroying the uinput device.
        Ok(())
    }
}

// ── Linux input group check ───────────────────────────────────

#[cfg(target_os = "linux")]
fn check_input_group() -> Result<(), String> {
    // Read /proc/self/status to get supplementary groups.
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|e| format!("Cannot read /proc/self/status: {e}"))?;

    let groups_line = status
        .lines()
        .find(|l| l.starts_with("Groups:"))
        .ok_or("Cannot find Groups in /proc/self/status")?;

    let sup_gids: Vec<u32> = groups_line
        .split_once(':')
        .map(|(_, v)| v)
        .unwrap_or("")
        .split_whitespace()
        .filter_map(|g| g.parse().ok())
        .collect();

    // Read /etc/group to find the input group's GID.
    let group_file = std::fs::read_to_string("/etc/group")
        .map_err(|e| format!("Cannot read /etc/group: {e}"))?;

    let input_gid: Option<u32> = group_file
        .lines()
        .filter_map(|line| {
            let mut parts = line.split(':');
            let name = parts.next()?;
            if name == "input" {
                parts.nth(1)?.parse().ok()
            } else {
                None
            }
        })
        .next();

    let input_gid = match input_gid {
        Some(g) => g,
        None => return Ok(()), // No 'input' group → not using uinput.
    };

    // Check supplementary groups.
    if sup_gids.contains(&input_gid) {
        return Ok(());
    }

    // Also check the primary group from /etc/passwd.
    let primary_gid = get_primary_gid();
    if primary_gid == Some(input_gid) {
        return Ok(());
    }

    Err(format!(
        "You are not in the 'input' group (GID {input_gid}).\n\
         Run: sudo usermod -aG input $USER\n\
         Then log out and back in for the change to take effect."
    ))
}

/// Get the current user's primary GID from /etc/passwd.
#[cfg(target_os = "linux")]
fn get_primary_gid() -> Option<u32> {
    // Get UID from /proc/self/status (no libc needed).
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let uid: u32 = status
        .lines()
        .find(|l| l.starts_with("Uid:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())?;

    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    for line in passwd.lines() {
        let mut parts = line.split(':');
        parts.next()?; // name
        parts.next()?; // password
        let uid_str = parts.next()?;
        if uid_str.parse::<u32>().ok() == Some(uid) {
            return parts.next()?.parse().ok(); // GID
        }
    }
    None
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "remote_input=info".into()),
        )
        .init();

    let args = Args::parse();
    let port = args.port;
    let paste_delay = Duration::from_millis(args.paste_delay);
    let use_http = args.http;
    let max_connections = args.max_connections;
    let allow = args.allow;

    // On Linux, verify the user is in the 'input' group before starting.
    #[cfg(target_os = "linux")]
    if let Err(e) = check_input_group() {
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), "\n  \x1b[1m\x1b[31mError:\x1b[0m {e}\n");
        std::process::exit(1);
    }

    let local_ip = detect_lan_ip();

    let (paste_tx, paste_rx) = mpsc::unbounded_channel::<String>();
    let connected: ConnectedIps = Arc::new(Mutex::new(HashMap::new()));

    std::thread::spawn(move || {
        paste_worker(paste_rx, paste_delay);
    });

    // ── Display startup box ────────────────────────────────────

    let url = if use_http {
        format!("http://{}:{}", local_ip, port)
    } else {
        format!("https://{}:{}", local_ip, port)
    };

    // ANSI color codes
    let bold = "\x1b[1m";
    let cyan = "\x1b[36m";
    let yellow = "\x1b[33m";
    let green = "\x1b[32m";
    let dim = "\x1b[2m";
    let reset = "\x1b[0m";

    eprintln!();
    eprintln!("  {bold}{cyan}Remote Input{reset}");
    eprintln!();
    eprintln!("  {green}Open on your phone:{reset}");
    eprintln!("  {green}→{reset} {bold}{url}{reset}");
    eprintln!();
    if use_http {
        eprintln!("  {yellow}⚠ Running in HTTP mode (unencrypted).{reset}");
        eprintln!("  {dim}  Traffic can be intercepted on the LAN.{reset}");
        eprintln!("  {dim}  Remove --http to use HTTPS instead.{reset}");
    } else {
        eprintln!("  {dim}The browser may warn that the certificate is untrusted.{reset}");
        eprintln!("  {dim}This is normal — it is a self-signed cert generated by{reset}");
        eprintln!("  {dim}this program. Your input is encrypted and private.{reset}");
    }
    eprintln!();
    eprintln!("  {dim}Scan QR code:{reset}");
    if let Ok(qr) = qr2term::generate_qr_string(&url) {
        for line in qr.lines() {
            eprintln!("  {line}");
        }
    }
    eprintln!();
    eprintln!("  {dim}Waiting for connection...{reset}");
    eprintln!();

    // ── Start server ───────────────────────────────────────────

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/assets/{*path}", get(serve_asset))
        .route("/ws", get(ws_handler))
        .with_state(AppState { paste_tx, connected, max_connections, allow });

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    if use_http {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
            .with_graceful_shutdown(shutdown_signal())
            .await?;
    } else {
        let tls_config = ensure_cert(local_ip).await?;
        let handle = axum_server::Handle::new();
        let shutdown_handle = handle.clone();
        tokio::spawn(async move {
            shutdown_signal().await;
            shutdown_handle.shutdown();
        });
        axum_server::bind_rustls(addr, tls_config)
            .handle(handle)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await?;
    }

    info!("Shutting down.");

    Ok(())
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

/// Check whitelist and capacity. Returns `Some(rejection_page)` if denied.
fn check_access(state: &AppState, ip: IpAddr) -> Option<axum::response::Response> {
    // Whitelist check.
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

    // Capacity check.
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
                conns.len(), state.max_connections,
            );
            return Some(reject_page(body));
        }
    }

    None
}

async fn serve_index(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> axum::response::Response {
    if let Some(rejection) = check_access(&state, addr.ip()) {
        return rejection;
    }
    serve_embedded("index.html")
}

async fn serve_asset(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    AxumPath(path): AxumPath<String>,
) -> Response {
    if let Some(rejection) = check_access(&state, addr.ip()) {
        return rejection;
    }
    // The wildcard captures everything after /assets/, so the file key
    // in the embedded directory is "assets/<path>".
    serve_embedded(&format!("assets/{path}"))
}

fn serve_embedded(path: &str) -> Response {
    match Frontend::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string();
            (
                [(axum::http::header::CONTENT_TYPE, mime)],
                content.data.to_vec(),
            )
                .into_response()
        }
        None => (axum::http::StatusCode::NOT_FOUND, "Not Found").into_response(),
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> axum::response::Response {
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
            warn!("Rejected connection from {ip}: limit reached ({}/{})", conns.len(), state.max_connections);
            // Still upgrade so the client gets a proper close frame with reason.
            return ws.on_upgrade(move |mut socket| async move {
                let _ = socket
                    .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                        code: 1013, // Try Again Later
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
                info!("Received {} chars from {ip}, pasting...", trimmed.chars().count());

                if let Err(e) = state.paste_tx.send(trimmed.to_string()) {
                    error!("Failed to send to paste worker: {e}");
                    break;
                }

                let ack = format!(r#"{{"ok":true,"len":{}}}"#, trimmed.chars().count());
                if socket.send(Message::Text(ack.into())).await.is_err() {
                    break;
                }
            }
            Message::Close(_) => {
                break;
            }
            _ => {}
        }
    }

    // Remove IP from connected set on disconnect.
    if let Ok(mut conns) = state.connected.lock() {
        conns.remove(&ip);
    }
    info!("Client {ip} disconnected");
}

// ── Platform-specific clipboard & paste ──────────────────────

#[cfg(unix)]
fn clipboard_set(text: &str, prev: &mut Option<std::process::Child>) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();

    if is_wayland {
        // Kill the previous wl-copy so we can own the selection cleanly.
        if let Some(mut child) = prev.take() {
            let _ = child.kill();
            let _ = child.wait();
            // Let the compositor process the disconnection before we
            // claim the selection with a new owner.
            std::thread::sleep(Duration::from_millis(10));
        }

        let mut child = Command::new("wl-copy")
            .args(["--foreground"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn wl-copy: {e}"))?;

        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .map_err(|e| format!("Failed to write to wl-copy stdin: {e}"))?;

        // Give wl-copy time to register as the Wayland selection owner.
        std::thread::sleep(Duration::from_millis(20));
        *prev = Some(child);
    } else {
        let mut child = Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn xclip: {e}"))?;

        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .map_err(|e| format!("Failed to write to xclip stdin: {e}"))?;

        let _ = child.wait();
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn paste_keystroke() -> Result<(), String> {
    uinput::paste_keystroke()
}

#[cfg(all(unix, not(target_os = "linux")))]
fn paste_keystroke() -> Result<(), String> {
    // Fallback for non-Linux Unix (e.g. macOS) — still use xdotool if available.
    use std::process::Command;
    let status = Command::new("xdotool")
        .args(["key", "ctrl+shift+v"])
        .status()
        .map_err(|e| format!("xdotool failed: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("xdotool exited with {status}"))
    }
}

#[cfg(windows)]
fn clipboard_set(text: &str, _prev: &mut Option<std::process::Child>) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("powershell")
        .args(["-NoProfile", "-Command", "$input | Set-Clipboard"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn powershell: {e}"))?;

    child
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())
        .map_err(|e| format!("Failed to write to powershell stdin: {e}"))?;

    let _ = child.wait();
    Ok(())
}

#[cfg(windows)]
fn paste_keystroke() -> Result<(), String> {
    use std::process::Command;
    let status = Command::new("powershell")
        .args(["-NoProfile", "-Command",
            "Add-Type -AssemblyName System.Windows.Forms; \
             [System.Windows.Forms.SendKeys]::SendWait('^v')"])
        .status()
        .map_err(|e| format!("Failed to run powershell: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("powershell exited with {status}"))
    }
}

/// Paste worker: clipboard write → sleep → simulate paste keystroke.
/// Unix: Ctrl+Shift+V (works in terminals and most GUI apps).
/// Windows: Ctrl+V.
fn paste_worker(mut rx: mpsc::UnboundedReceiver<String>, delay: Duration) {
    let mut clipboard_child: Option<std::process::Child> = None;

    while let Some(text) = rx.blocking_recv() {
        if let Err(e) = clipboard_set(&text, &mut clipboard_child) {
            error!("Clipboard write failed: {e}");
            continue;
        }

        std::thread::sleep(delay);

        if let Err(e) = paste_keystroke() {
            error!("Paste keystroke failed: {e}");
            continue;
        }

        info!("Pasted successfully");
    }

    // Clean up the last wl-copy on shutdown.
    if let Some(mut child) = clipboard_child {
        let _ = child.kill();
        let _ = child.wait();
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received");
}
