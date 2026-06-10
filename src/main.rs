use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;
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
}

/// Tracks which client IPs are currently connected.
type ConnectedIps = Arc<Mutex<HashMap<IpAddr, ()>>>;

/// Shared application state
#[derive(Clone)]
struct AppState {
    paste_tx: mpsc::UnboundedSender<String>,
    connected: ConnectedIps,
    max_connections: usize,
}

const INDEX_HTML: &str = include_str!("index.html");

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

    let w = 54; // inner width of the box
    eprintln!();
    eprintln!("  ╔{}╗", "═".repeat(w));
    eprintln!("  ║ {:<width$} ║", "Remote Input", width = w - 2);
    eprintln!("  ║ {:<width$} ║", "", width = w - 2);
    eprintln!("  ║ {:<width$} ║", "Open on your phone:", width = w - 2);
    eprintln!("  ║ → {:<width$} ║", url, width = w - 4);
    eprintln!("  ║ {:<width$} ║", "", width = w - 2);
    if use_http {
        eprintln!("  ║ {:<width$} ║", "⚠  Running in HTTP mode (unencrypted).", width = w - 2);
        eprintln!("  ║ {:<width$} ║", "   Traffic can be intercepted on the LAN.", width = w - 2);
        eprintln!("  ║ {:<width$} ║", "   Remove --http to use HTTPS instead.", width = w - 2);
    } else {
        eprintln!("  ║ {:<width$} ║", "The browser may warn that the certificate", width = w - 2);
        eprintln!("  ║ {:<width$} ║", "is untrusted. This is normal because it", width = w - 2);
        eprintln!("  ║ {:<width$} ║", "is a temporary self-signed cert generated", width = w - 2);
        eprintln!("  ║ {:<width$} ║", "by this program. It ensures your input is", width = w - 2);
        eprintln!("  ║ {:<width$} ║", "encrypted and cannot be viewed by others.", width = w - 2);
    }
    eprintln!("  ║ {:<width$} ║", "", width = w - 2);
    eprintln!("  ║ {:<width$} ║", "Waiting for connection...", width = w - 2);
    eprintln!("  ╚{}╝", "═".repeat(w));
    eprintln!();

    // ── Start server ───────────────────────────────────────────

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/ws", get(ws_handler))
        .with_state(AppState { paste_tx, connected, max_connections });

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

async fn serve_index(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> axum::response::Response {
    let ip = addr.ip();
    {
        let conns = state.connected.lock().unwrap();
        if !conns.contains_key(&ip) && conns.len() >= state.max_connections {
            warn!("Rejected page request from {ip}: limit reached");
            let msg = format!(
                "Server is at capacity ({}/{}). To allow more connections, restart with:\n\n    remote-input -m <number>\n\nFor example:\n\n    remote-input -m 3\n",
                conns.len(), state.max_connections,
            );
            return (axum::http::StatusCode::FORBIDDEN, msg).into_response();
        }
    }
    Html(INDEX_HTML).into_response()
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> axum::response::Response {
    let ip = addr.ip();

    // Check connection limit before upgrading.
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

#[cfg(unix)]
fn paste_keystroke() -> Result<(), String> {
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
