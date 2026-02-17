# System Architecture: RustyGate

## 1. Process Model
RustyGate runs as a single process with two primary components:
1.  **Main Thread (Web Server)**: Hosts an Axum-based web server and Dioxus Web UI.
2.  **Tokio Runtime (Core)**: Handles non-blocking I/O (BACnet, MQTT, SQLite).

## 2. Inter-Task Communication (ITC)
- **UI -> Core**: REST API or WebSockets for commands (e.g., `BindInterface`).
- **Core -> UI**: WebSockets for real-time telemetry and logs.

## 3. Data Flow
1.  **Discovery**: 
    - **Phase 1 (Network)**: UI sends `StartDiscovery` -> Core triggers `Who-Is` via `bacnet-rs` -> Core receives `I-Am` -> Core broadcasts `DeviceDiscovered`.
    - **Phase 2 (Objects)**: Upon discovering a new device, Core automatically triggers `ReadProperty(ObjectList)` -> Receives list of Object Identifiers -> Broadcasts `DeviceObjectsDiscovered`. This ensures a seamless "one-click" discovery experience in the UI.
2.  **Polling**: Core periodically reads BACnet properties -> formats as JSON -> publishes to MQTT broker via `rumqttc` -> broadcasts `PointUpdate` to UI for monitoring.
3.  **Command & Control**:
    - **Shutdown**: A global atomic flag is used to signal a graceful exit. The BACnet receiver thread uses a socket timeout to periodically check this flag, ensuring the process exits cleanly on `Ctrl+C`.

## 4. Module Responsibilities
- `src/main.rs`: Entry point, channel initialization, and thread management.
- `src/core/`: The "Engine". Handles protocol logic and I/O.
    - `network/`: Interface discovery and socket binding.
- `src/ui/`: The "View". Renders configuration and monitoring state.
- `src/common/`: Shared types and constants used by both Core and UI.

## 5. Testing Strategy (Remote Responder)
- **Tooling**: A dedicated `bacnet-responder` simulator (located in `tests/bacnet-responder`).
- **Functionality**: Acts as a "Ghost Device" that listens on the network and responds to BACnet services.
- **Objects**: Simulates multiple standard objects including Analog Input (AI), Binary Input (BI), and Analog Value (AV).
- **Services**: Supports `Who-Is` (unconfirmed) and `ReadProperty` (confirmed) for object lists and property values.
- **Orchestration**: Controllable via MQTT on topic `test/ghost/config` to dynamically change its Device ID or object values.
- **Verification**: Used to verify Gateway discovery (`Who-Is`), object enumeration, and polling without requiring physical hardware.
