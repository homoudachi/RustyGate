# System Architecture: RustyGate

## 1. Process Model
RustyGate runs as a single process with two primary components:
1.  **Main Thread (Web Server)**: Hosts an Axum-based web server and Dioxus Web UI.
2.  **Tokio Runtime (Core)**: Handles non-blocking I/O (BACnet, MQTT, SQLite).

## 2. Inter-Task Communication (ITC)
- **UI -> Core**: REST API or WebSockets for commands (e.g., `BindInterface`).
- **Core -> UI**: WebSockets for real-time telemetry and logs.

## 3. Data Flow
1.  **Discovery**: UI sends `StartDiscovery` -> Core triggers `Who-Is` via `bacnet-rs` -> Core receives `I-Am` -> Core broadcasts `DeviceDiscovered` -> UI updates device list.
2.  **Polling**: Core periodically reads BACnet properties -> formats as JSON -> publishes to MQTT broker via `rumqttc` -> broadcasts `PointUpdate` to UI for monitoring.
3.  **Command**: MQTT subscriber or UI sends `WriteProperty` -> Core validates and sends BACnet `WriteProperty` request.

## 4. Module Responsibilities
- `src/main.rs`: Entry point, channel initialization, and thread management.
- `src/core/`: The "Engine". Handles protocol logic and I/O.
    - `network/`: Interface discovery and socket binding.
- `src/ui/`: The "View". Renders configuration and monitoring state.
- `src/common/`: Shared types and constants used by both Core and UI.

## 5. Testing Strategy (Remote Responder)
- **Tooling**: A dedicated `bacnet-responder` simulator.
- **Orchestration**: Automated test runners using MQTT to manipulate the responder's state and verify gateway translation.
- **Verification**: Strict validation of COV thresholds, polling intervals, and Priority Array logic.
