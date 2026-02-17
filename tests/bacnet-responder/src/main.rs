use bacnet_rs::{
    app::Apdu,
    datalink::{bip::BacnetIpDataLink, DataLink, DataLinkAddress},
    object::{ObjectIdentifier, ObjectType},
    service::{IAmRequest, UnconfirmedServiceChoice, WhoIsRequest},
};
use rumqttc::{AsyncClient, MqttOptions, QoS, Event as MqttEvent, Packet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, sleep};
use std::env;

struct ResponderState {
    device_instance: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let device_id = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(99999);
    let mqtt_host = args.get(2).map(|s| s.as_str()).unwrap_or("localhost");

    log::info!("Starting BACnet Ghost Device (ID: {}, MQTT: {})", device_id, mqtt_host);

    let state = Arc::new(Mutex::new(ResponderState {
        device_instance: device_id,
    }));

    let _mqtt_state = Arc::clone(&state);
    // Non-blocking MQTT attempt
    tokio::spawn(async move {
        let mut mqttoptions = MqttOptions::new(format!("bacnet-ghost-{}", device_id), mqtt_host, 1883);
        mqttoptions.set_keep_alive(Duration::from_secs(5));
        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
        
        let _ = client.subscribe("test/ghost/config", QoS::AtLeastOnce).await;
        loop {
            match eventloop.poll().await {
                Ok(notification) => {
                    if let MqttEvent::Incoming(Packet::Publish(publish)) = notification {
                        log::info!("Ghost received MQTT command: {:?}", publish.payload);
                    }
                }
                Err(_) => {
                    sleep(Duration::from_secs(10)).await;
                }
            }
        }
    });

    let bind_addr: SocketAddr = "0.0.0.0:47808".parse()?; 
    let mut datalink = BacnetIpDataLink::new(bind_addr)?;
    log::info!("Ghost Device listening for BACnet on {}", bind_addr);

    loop {
        match datalink.receive_frame() {
            Ok((data, src_addr)) => {
                if let Ok(apdu) = Apdu::decode(&data) {
                    if let Apdu::UnconfirmedRequest { service_choice, service_data } = apdu {
                        if service_choice == UnconfirmedServiceChoice::WhoIs as u8 {
                            if let Ok(who_is) = WhoIsRequest::decode(&service_data) {
                                let s = state.lock().unwrap();
                                if who_is.matches(s.device_instance) {
                                    log::info!("Received Who-Is from {:?}. Sending I-Am...", src_addr);
                                    
                                    let iam = IAmRequest::new(
                                        ObjectIdentifier::new(ObjectType::Device, s.device_instance),
                                        1476,
                                        0,
                                        999,
                                    );
                                    
                                    let mut iam_data = Vec::new();
                                    iam.encode(&mut iam_data).map_err(|e| anyhow::anyhow!(e.to_string()))?;
                                    
                                    let response_apdu = Apdu::UnconfirmedRequest {
                                        service_choice: UnconfirmedServiceChoice::IAm as u8,
                                        service_data: iam_data,
                                    };
                                    
                                    let encoded_response = response_apdu.encode();
                                    datalink.send_frame(&encoded_response, &DataLinkAddress::Broadcast)?;
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let err_str = e.to_string();
                // Resource temporarily unavailable is EAGAIN/WouldBlock
                if !err_str.contains("TimedOut") && !err_str.contains("WouldBlock") && !err_str.contains("Resource temporarily unavailable") {
                    log::error!("Datalink error: {}", e);
                }
                // Sleep briefly on any error (including WouldBlock) to prevent spinning
                sleep(Duration::from_millis(100)).await;
            }
        }
        tokio::task::yield_now().await;
    }
}
