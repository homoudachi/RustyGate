use crate::common::types::{Command, Event, BacnetDevice, BacnetObjectInfo};
use crate::core::network::interface;
use tokio::sync::{mpsc, broadcast, Mutex as TokioMutex};
use axum::routing::{get, post};
use axum::{Json, Router, extract::State, response::IntoResponse, response::Sse, response::sse::{Event as SseEvent, KeepAlive}};
use std::net::SocketAddr;
use std::sync::Arc;
use futures::stream::Stream;
use std::collections::HashMap;


struct AppState {
    cmd_tx: mpsc::Sender<Command>,
    event_tx: broadcast::Sender<Event>,
    discovered_devices: TokioMutex<HashMap<u32, BacnetDevice>>,
    device_objects: TokioMutex<HashMap<u32, Vec<BacnetObjectInfo>>>,
}

pub async fn launch(cmd_tx: mpsc::Sender<Command>, event_tx: broadcast::Sender<Event>) {
    let state = Arc::new(AppState { 
        cmd_tx, 
        event_tx: event_tx.clone(),
        discovered_devices: TokioMutex::new(HashMap::new()),
        device_objects: TokioMutex::new(HashMap::new()),
    });

    // Spawn a task to update discovered devices from events
    let mut event_rx = event_tx.subscribe();
    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match event {
                Event::DeviceDiscovered(dev) => {
                    let mut devices = state_clone.discovered_devices.lock().await;
                    let is_new = !devices.contains_key(&dev.instance);
                    devices.insert(dev.instance, dev.clone());
                    
                    if is_new {
                        // Check if we already have objects (to avoid re-scanning)
                        let objects = state_clone.device_objects.lock().await;
                        if !objects.contains_key(&dev.instance) {
                            log::info!("Auto-discovering objects for new device {}", dev.instance);
                            let _ = state_clone.cmd_tx.send(Command::DiscoverObjects {
                                interface: "".to_string(),
                                device_id: dev.instance,
                                address: dev.address.clone(),
                            }).await;
                        }
                    }
                }
                Event::DeviceObjectsDiscovered { device_id, objects } => {
                    let mut all_objects = state_clone.device_objects.lock().await;
                    all_objects.insert(device_id, objects);
                }
                _ => {}
            }
        }
    });

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/interfaces", get(list_interfaces))
        .route("/api/bind", post(bind_interface))
        .route("/api/discover", post(start_discovery))
        .route("/api/ping", post(ping_handler))
        .route("/api/write", post(write_handler))
        .route("/api/devices", get(get_devices))
        .route("/api/devices/:id/objects", get(get_device_objects))
        .route("/api/events", get(events_handler))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    log::info!("Web UI server listening on http://localhost:8080");
    
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            log::error!("Failed to bind Web UI to {}: {}. Is another instance running?", addr, e);
            return;
        }
    };
    axum::serve(listener, app).await.unwrap();
}

async fn index_handler() -> impl IntoResponse {
    axum::response::Html(include_str!("index.html"))
}

async fn list_interfaces() -> impl IntoResponse {
    match interface::list_interfaces() {
        Ok(ifaces) => {
            let names: Vec<String> = ifaces.into_iter().map(|i| i.name).collect();
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

async fn start_discovery(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let _ = state.cmd_tx.send(Command::StartDiscovery).await;
    Json("Discovery started")
}

#[derive(serde::Deserialize)]
struct PingRequest {
    target_ip: String,
}

async fn ping_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PingRequest>,
) -> impl IntoResponse {
    let _ = state.cmd_tx.send(Command::Ping { 
        interface: "".to_string(), // Use current binding
        target: payload.target_ip 
    }).await;
    Json("Ping sent")
}

#[derive(serde::Deserialize)]
struct WriteRequest {
    device_id: u32,
    address: String,
    object_type: u16,
    instance: u32,
    property: u32,
    value: String,
}

async fn write_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WriteRequest>,
) -> impl IntoResponse {
    let _ = state.cmd_tx.send(Command::WriteProperty {
        device_id: payload.device_id,
        address: payload.address,
        object_type: payload.object_type,
        instance: payload.instance,
        property: payload.property,
        value: payload.value,
    }).await;
    Json("Write requested")
}

async fn get_devices(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let devices = state.discovered_devices.lock().await;
    Json(devices.values().cloned().collect::<Vec<_>>())
}

async fn get_device_objects(
    axum::extract::Path(id): axum::extract::Path<u32>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let objects = state.device_objects.lock().await;
    match objects.get(&id) {
        Some(objs) => Json(objs.clone()),
        None => {
            // Trigger object discovery if not found
            let devices = state.discovered_devices.lock().await;
            if let Some(dev) = devices.get(&id) {
                let _ = state.cmd_tx.send(Command::DiscoverObjects {
                    interface: "".to_string(), // Core will use current bound interface if empty
                    device_id: id,
                    address: dev.address.clone(),
                }).await;
            }
            Json(vec![])
        }
    }
}

async fn events_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<SseEvent, std::convert::Infallible>>> {
    let mut rx = state.event_tx.subscribe();

    let stream = async_stream::stream! {
        while let Ok(event) = rx.recv().await {
            yield Ok(SseEvent::default().json_data(event).unwrap());
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}
