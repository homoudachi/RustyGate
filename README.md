# RustyGate: BACnet to MQTT Gateway

A high-performance, robust BACnet to MQTT gateway written in Rust.

## Features
- **Device Discovery**: Phase 1 network discovery via `Who-Is` / `I-Am`.
- **Object Discovery**: Phase 2 object enumeration via `ReadProperty(ObjectList)`.
- **Live Monitoring**: Web-based UI for real-time telemetry.
- **MQTT Integration**: Reliable point updates to MQTT brokers.
- **Simulator**: Built-in BACnet simulator for testing without hardware.

## Quick Start

### 1. Build
```bash
cargo build
```

### 2. Run Gateway
```bash
cargo run -- <interface_name>
```

### 3. CLI Tools
- **List Interfaces**: `cargo run -- list`
- **Ping Device**: `cargo run -- ping <interface> <target_ip>`
- **Discover Objects**: `cargo run -- discover-objects <interface> <device_id> <target_ip:port>`

## Testing

Use the provided integration scripts:
- **Discovery Test**: `./scripts/test_remote.sh <local_iface> <remote_iface>`
- **Object Test**: `./scripts/test_objects.sh <local_iface> <remote_iface>`

## Documentation
- [System Architecture](docs/architecture.md)
- [Simulator Guide](docs/simulator.md)
- [Functional Specification](docs/functional-specification-document.md)
