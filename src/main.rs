use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use clap::Parser;
use tokio::sync::mpsc;
use tracing::{error, info};

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
}

/// Shared application state
#[derive(Clone)]
struct AppState {
    paste_tx: mpsc::UnboundedSender<String>,
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

    let local_ip = local_ip_address::local_ip().unwrap_or(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));

    let url = format!("http://{}:{}", local_ip, port);
    let w = 38; // inner width of the box
    eprintln!();
    eprintln!("  ╔{}╗", "═".repeat(w));
    eprintln!("  ║ {:<width$} ║", "Remote Input", width = w - 2);
    eprintln!("  ║ {:<width$} ║", "", width = w - 2);
    eprintln!("  ║ {:<width$} ║", "Open on your phone:", width = w - 2);
    eprintln!("  ║ → {:<width$} ║", url, width = w - 4);
    eprintln!("  ║ {:<width$} ║", "", width = w - 2);
    eprintln!("  ║ {:<width$} ║", "Waiting for connection...", width = w - 2);
    eprintln!("  ╚{}╝", "═".repeat(w));
    eprintln!();

    let (paste_tx, paste_rx) = mpsc::unbounded_channel::<String>();

    std::thread::spawn(move || {
        paste_worker(paste_rx, paste_delay);
    });

    let state = AppState { paste_tx };

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Shutting down.");

    Ok(())
}

async fn serve_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    info!("Client connected");

    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            Message::Text(text) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                info!("Received {} chars, pasting...", trimmed.len());

                if let Err(e) = state.paste_tx.send(trimmed.to_string()) {
                    error!("Failed to send to paste worker: {}", e);
                    break;
                }

                let ack = format!(r#"{{"ok":true,"len":{}}}"#, trimmed.chars().count());
                if socket.send(Message::Text(ack.into())).await.is_err() {
                    break;
                }
            }
            Message::Close(_) => {
                info!("Client disconnected");
                break;
            }
            _ => {}
        }
    }

    info!("WebSocket connection closed");
}

/// Write text to system clipboard.
///
/// Wayland: a single `wl-copy --foreground` process is spawned once and kept
/// alive — it holds selection ownership for the lifetime of the program.
/// Each new write kills the old process, spawns a fresh one, and waits briefly
/// for the compositor to settle before returning.
///
/// X11: `xclip` is spawned per write (clipboard persists after exit).
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

/// Simulate Ctrl+Shift+V via xdotool.
/// Ctrl+Shift+V works as paste in both terminals and most GUI apps on Linux.
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

/// Paste worker: clipboard write → sleep → simulate Ctrl+Shift+V.
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
