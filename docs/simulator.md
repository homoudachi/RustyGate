# BACnet Simulator (bacnet-responder)

The `bacnet-responder` is a dedicated tool for testing the RustyGate gateway without requiring physical BACnet hardware. It simulates a functional BACnet/IP device with multiple objects.

## Features
- **Device Simulation**: Mimics a standard BACnet/IP device.
- **Object Support**:
    - **Analog Input (AI)**: Simulates sensors (e.g., Temperature).
    - **Binary Input (BI)**: Simulates digital status (e.g., Fan on/off).
    - **Analog Value (AV)**: Software variables.
- **Dynamic Values**: Values (like "Room Temperature") increment automatically to simulate real-world changes.
- **Service Support**:
    - `Who-Is` / `I-Am`: For network discovery.
    - `ReadProperty`: Supports reading:
        - `ObjectList` (for object discovery)
        - `PresentValue`
        - `ObjectName`
        - `ObjectType`
        - `StatusFlags`

## Running the Simulator

### Local Test
```bash
cd tests/bacnet-responder
cargo run -- <device_id> [mqtt_host] [interface_name]
```

### Remote Test
Use the provided scripts from the project root:
```bash
./scripts/test_objects.sh <local_iface> <remote_iface>
```

## Protocol Implementation Details
The simulator uses the `bacnet-rs` library but implements its own custom request/response handlers for confirmed services like `ReadProperty`, as the base library is primarily focused on client-side operations.

### Unicast Responses
To improve reliability in complex WiFi/Docker networks, the simulator responds to `Who-Is` requests with unicast `I-Am` packets sent directly to the requester's IP, bypassing potential broadcast blocking.
