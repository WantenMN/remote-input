use axum::extract::connect_info::ConnectInfo;
use axum::extract::{Path as AxumPath, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rust_embed::Embed;
use std::net::SocketAddr;

use crate::state::{check_access, AppState};
use crate::ws;

#[derive(Embed)]
#[folder = "web-dist/"]
struct Frontend;

/// Build the Axum router with all routes.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/assets/{*path}", get(serve_asset))
        .route("/ws", get(ws::handler))
        .with_state(state)
}

async fn serve_index(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Response {
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
