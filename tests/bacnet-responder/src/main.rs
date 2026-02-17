use bacnet_rs::{
    app::Apdu,
    datalink::{bip::BacnetIpDataLink, DataLink, DataLinkAddress},
    object::{ObjectIdentifier, ObjectType},
    service::{IAmRequest, UnconfirmedServiceChoice, WhoIsRequest},
};
use rumqttc::{AsyncClient, MqttOptions, QoS, Event as MqttEvent, Packet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, atomic::{AtomicU32, Ordering}};
use tokio::time::{Duration, sleep};
use std::env;

struct ResponderState {
    device_instance: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let device_id = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(99999);
    let mqtt_host = args.get(2).cloned().unwrap_or_else(|| "localhost".to_string());
    let iface_name = args.get(3).cloned();

    log::info!("Starting BACnet Ghost Device (ID: {}, MQTT: {}, Interface: {:?})", 
        device_id, mqtt_host, iface_name);

    let state = Arc::new(Mutex::new(ResponderState {
        device_instance: device_id,
    }));

    let error_count = Arc::new(AtomicU32::new(0));

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
                Err(e) => {
                    log::warn!("MQTT error: {}. Retrying in 10s...", e);
                    sleep(Duration::from_secs(10)).await;
                }
            }
        }
    });

    let bind_addr: SocketAddr = if let Some(name) = iface_name {
        let addrs = if_addrs::get_if_addrs()?;
        let iface = addrs.into_iter()
            .find(|i| i.name == name && i.addr.ip().is_ipv4())
            .ok_or_else(|| anyhow::anyhow!("Interface {} not found or has no IPv4", name))?;
        SocketAddr::new(iface.addr.ip(), 47808)
    } else {
        "0.0.0.0:47808".parse()?
    };

    let mut datalink = match BacnetIpDataLink::new(bind_addr) {
        Ok(d) => d,
        Err(e) => {
            log::error!("Failed to bind BACnet datalink on {}: {}", bind_addr, e);
            return Err(e.into());
        }
    };
    log::info!("Ghost Device listening for BACnet on {}", bind_addr);

    let mut last_who_is_log = tokio::time::Instant::now() - Duration::from_secs(10);

    loop {
        match datalink.receive_frame() {
            Ok((data, src_addr)) => {
                error_count.store(0, Ordering::Relaxed); // Reset error count on success
                if let Ok(apdu) = Apdu::decode(&data) {
                    if let Apdu::UnconfirmedRequest { service_choice, service_data } = apdu {
                        if service_choice == UnconfirmedServiceChoice::WhoIs as u8 {
                            if let Ok(who_is) = WhoIsRequest::decode(&service_data) {
                                let s = state.lock().unwrap();
                                if who_is.matches(s.device_instance) {
                                    if last_who_is_log.elapsed() > Duration::from_secs(5) {
                                        log::info!("Received Who-Is from {:?}. Sending I-Am... (rate-limited)", src_addr);
                                        last_who_is_log = tokio::time::Instant::now();
                                    }
                                    
                                    let iam = IAmRequest::new(
                                        ObjectIdentifier::new(ObjectType::Device, s.device_instance),
                                        1476,
                                        0,
                                        999,
                                    );
                                    
                                    let mut iam_data = Vec::new();
                                    if let Err(e) = iam.encode(&mut iam_data) {
                                        log::error!("Failed to encode I-Am: {}", e);
                                        continue;
                                    }
                                    
                                    let response_apdu = Apdu::UnconfirmedRequest {
                                        service_choice: UnconfirmedServiceChoice::IAm as u8,
                                        service_data: iam_data,
                                    };
                                    
                                    let encoded_response = response_apdu.encode();
                                    if let Err(e) = datalink.send_frame(&encoded_response, &src_addr) {
                                        log::error!("Failed to send I-Am: {}", e);
                                    }
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
                    let count = error_count.fetch_add(1, Ordering::Relaxed);
                    if count == 0 || count % 100 == 0 {
                        log::error!("Datalink error (count: {}): {}", count + 1, e);
                    }
                }
                // Sleep briefly on any error (including WouldBlock) to prevent spinning
                sleep(Duration::from_millis(100)).await;
            }
        }
        tokio::task::yield_now().await;
    }
}
