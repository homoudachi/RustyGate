use bacnet_rs::datalink::bip::BacnetIpDataLink;
use bacnet_rs::object::{Device, ObjectIdentifier, ObjectType, PropertyIdentifier};
use bacnet_rs::service::{IAmRequest, WhoIsRequest};
use rumqttc::{AsyncClient, MqttOptions, QoS, Event as MqttEvent, Packet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::time::Duration;

struct ResponderState {
    device: Device,
    // Add simulated objects here
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    log::info!("Starting BACnet Ghost Device (Responder)...");

    let state = Arc::new(Mutex::new(ResponderState {
        device: Device::new(99999, "Ghost-Device-1".to_string()),
    }));

    // MQTT Control Setup
    let mut mqttoptions = MqttOptions::new("bacnet-ghost-1", "localhost", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    let mqtt_state = Arc::clone(&state);
    tokio::spawn(async move {
        client.subscribe("test/ghost/set_property", QoS::AtLeastOnce).await.unwrap();
        loop {
            if let Ok(notification) = eventloop.poll().await {
                if let MqttEvent::Incoming(Packet::Publish(publish)) = notification {
                    log::info!("Ghost received control command: {:?}", publish.payload);
                    // Update internal BACnet state based on MQTT command
                }
            }
        }
    });

    // BACnet UDP Listener
    let bind_addr: SocketAddr = "0.0.0.0:47809".parse()?; // Use different port for local testing
    let datalink = BacnetIpDataLink::new(bind_addr)?;
    log::info!("Ghost Device listening on {}", bind_addr);

    // Basic loop to handle Who-Is
    loop {
        // Here we will use bacnet-rs to receive and respond to packets.
        // For now, we are just keeping the process alive.
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
