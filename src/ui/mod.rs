use crate::common::types::{Command, Event};
use crate::core::network::interface;
use tokio::sync::{mpsc, broadcast};
use axum::routing::{get, post};
use axum::{Json, Router, extract::State, response::IntoResponse};
use std::net::SocketAddr;
use std::sync::Arc;

struct AppState {
    cmd_tx: mpsc::Sender<Command>,
    event_tx: broadcast::Sender<Event>,
}

pub async fn launch(cmd_tx: mpsc::Sender<Command>, event_tx: broadcast::Sender<Event>) {
    let state = Arc::new(AppState { cmd_tx, event_tx });

    let app = Router::new()
        .route("/api/interfaces", get(list_interfaces))
        .route("/api/bind", post(bind_interface))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    log::info!("Web UI server listening on {}", addr);
    
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            log::error!("Failed to bind Web UI to {}: {}. Is another instance running?", addr, e);
            return;
        }
    };
    axum::serve(listener, app).await.unwrap();
}

async fn list_interfaces() -> impl IntoResponse {
    match interface::list_interfaces() {
        Ok(ifaces) => {
            let names: Vec<String> = ifaces.into_iter().map(|i| format!("{} ({})", i.name, i.ip)).collect();
            Json(names)
        }
        Err(_) => Json(vec![]),
    }
}

#[derive(serde::Deserialize)]
struct BindRequest {
    interface_name: String,
}

async fn bind_interface(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BindRequest>,
) -> impl IntoResponse {
    let _ = state.cmd_tx.send(Command::BindInterface(payload.interface_name)).await;
    Json("Binding requested")
}
