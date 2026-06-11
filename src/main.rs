use std::net::SocketAddr;
use std::time::Duration;

use clap::Parser;
use tokio::sync::mpsc;
use tracing::info;

use remote_input::cli::Args;
use remote_input::state::{AppState, ConnectedIps};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "remote_input=info".into()),
        )
        .init();

    let args = Args::parse();
    let paste_delay = Duration::from_millis(args.paste_delay);

    #[cfg(target_os = "linux")]
    if let Err(e) = remote_input::paste::linux_check::check_input_group() {
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), "\n  \x1b[1m\x1b[31mError:\x1b[0m {e}\n");
        std::process::exit(1);
    }

    let local_ip = remote_input::net::detect_lan_ip();
    let (paste_tx, paste_rx) = mpsc::unbounded_channel::<String>();
    let connected: ConnectedIps = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));

    std::thread::spawn(move || remote_input::paste::run(paste_rx, paste_delay));

    remote_input::startup::print(local_ip, args.port, args.http);

    let state = AppState {
        paste_tx,
        connected,
        max_connections: args.max_connections,
        allow: args.allow,
    };
    let app = remote_input::server::build_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));

    if args.http {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
            .with_graceful_shutdown(shutdown_signal())
            .await?;
    } else {
        let tls_config = remote_input::tls::ensure_cert(local_ip).await?;
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
