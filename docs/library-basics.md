# Library Basics: bacnet-rs and rumqttc

This document provides a basic overview of the primary libraries used in the RustyGate project.

## 1. bacnet-rs
The `bacnet-rs` library is a Rust implementation of the BACnet protocol stack.

### Device Creation
A BACnet device is represented by the `Device` struct.
```rust
use bacnet_rs::object::{Device, ObjectIdentifier, ObjectType};

let mut device = Device::new(12345, "My Gateway".to_string());
device.vendor_name = "RustyGate".to_string();
```

### BACnet/IP DataLink
To communicate over the network using BACnet/IP, use `BacnetIpDataLink`.
```rust
use bacnet_rs::datalink::bip::BacnetIpDataLink;
use std::net::SocketAddr;

let bind_addr: SocketAddr = "0.0.0.0:47808".parse().unwrap();
let datalink = BacnetIpDataLink::new(bind_addr).unwrap();
```

### Services and APDUs
BACnet services are handled via Application Protocol Data Units (APDUs).
- **Who-Is**: Used for device discovery.
- **I-Am**: Response to Who-Is.
- **ReadProperty**: Read a specific property of an object.

Example of creating a Who-Is request:
```rust
use bacnet_rs::service::WhoIsRequest;
let whois = WhoIsRequest::for_device(12345);
```

## 2. rumqttc
`rumqttc` is an asynchronous MQTT client for Rust.

### Client and EventLoop
The application uses an `AsyncClient` and an `EventLoop`. The `AsyncClient` is used to send requests (publish, subscribe), and the `EventLoop` is used to poll for incoming messages and network events.

```rust
use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::time::Duration;

let mut mqttoptions = MqttOptions::new("gateway-1", "localhost", 1883);
mqttoptions.set_keep_alive(Duration::from_secs(5));

let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
```

### Publishing and Subscribing
```rust
// Subscribe
client.subscribe("bacnet/+/+/+", QoS::AtLeastOnce).await.unwrap();

// Publish
client.publish("bacnet/1/2/3/4", QoS::AtLeastOnce, false, payload).await.unwrap();
```

### Handling Events
Incoming messages and connection status are handled by polling the event loop.
```rust
loop {
    let event = eventloop.poll().await;
    match event {
        Ok(notification) => {
            // Process notification (e.g., Incoming(Publish))
        }
        Err(e) => {
            // Handle error (e.g., disconnection)
        }
    }
}
```

## 3. Integration Strategy
- **Tokio Runtime**: Both libraries will run within a Tokio runtime.
- **Channels**: Use `mpsc` channels to communicate between the MQTT event loop, BACnet engine, and the UI.
- **State Management**: SQLite via `sqlx` will store configuration and discovered points.
