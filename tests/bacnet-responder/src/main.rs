use bacnet_rs::{
    app::Apdu,
    datalink::{bip::BacnetIpDataLink, DataLink},
    object::{
        analog::AnalogInput,
        binary::BinaryInput,
        database::ObjectDatabase,
        Device, ObjectIdentifier, ObjectType, PropertyIdentifier, PropertyValue,
    },
    service::{IAmRequest, UnconfirmedServiceChoice, WhoIsRequest, ReadPropertyRequest},
    encoding::{
        self,
        encode_context_object_id, encode_context_enumerated,
    },
};
use rumqttc::{AsyncClient, MqttOptions, QoS, Event as MqttEvent, Packet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, atomic::{AtomicU32, Ordering}};
use tokio::time::{Duration, sleep};
use std::env;

struct ResponderState {
    device_instance: u32,
    db: ObjectDatabase,
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
    let gateway_ip = args.get(4).cloned();

    log::info!("Starting BACnet Ghost Device (ID: {}, MQTT: {}, Interface: {:?}, Gateway IP: {:?})", 
        device_id, mqtt_host, iface_name, gateway_ip);

    // Initialize Database with Device object
    let device = Device::new(device_id, format!("Ghost Device {}", device_id));
    let db = ObjectDatabase::new(device);

    // Add some objects
    let mut ai1 = AnalogInput::new(1, "Room Temperature".to_string());
    ai1.set_present_value(22.5);
    db.add_object(Box::new(ai1)).unwrap();

    let mut ai2 = AnalogInput::new(2, "Outdoor Temperature".to_string());
    ai2.set_present_value(10.0);
    db.add_object(Box::new(ai2)).unwrap();

    let mut av1 = AnalogInput::new(10, "Temperature Setpoint".to_string()); // High instance for AV simulation
    av1.set_present_value(21.0);
    db.add_object(Box::new(av1)).unwrap();

    let bi1 = BinaryInput::new(1, "Fan Status".to_string());
    db.add_object(Box::new(bi1)).unwrap();

    let state = Arc::new(Mutex::new(ResponderState {
        device_instance: device_id,
        db,
    }));

    let error_count = Arc::new(AtomicU32::new(0));

    let state_clone = Arc::clone(&state);
    // Simulation task: update values periodically
    tokio::spawn(async move {
        let mut temp = 22.5;
        let mut out_temp = 10.0;
        loop {
            sleep(Duration::from_secs(2)).await;
            temp += 0.1;
            if temp > 25.0 { temp = 20.0; }
            out_temp += 0.05;
            if out_temp > 15.0 { out_temp = 5.0; }
            
            let s = state_clone.lock().unwrap();
            let ai1_id = ObjectIdentifier::new(ObjectType::AnalogInput, 1);
            let _ = s.db.set_property(ai1_id, PropertyIdentifier::PresentValue, PropertyValue::Real(temp));
            
            let ai2_id = ObjectIdentifier::new(ObjectType::AnalogInput, 2);
            let _ = s.db.set_property(ai2_id, PropertyIdentifier::PresentValue, PropertyValue::Real(out_temp));
        }
    });

    // Non-blocking MQTT attempt
    let _mqtt_state = Arc::clone(&state);
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

    let bind_addr: SocketAddr = "0.0.0.0:47808".parse()?;
    
    // We can still use iface_name to log which IP we are simulating
    if let Some(name) = iface_name {
        let addrs = if_addrs::get_if_addrs()?;
        if let Some(iface) = addrs.into_iter().find(|i| i.name == name && i.addr.ip().is_ipv4()) {
            log::info!("Simulating device on interface {} (IP: {})", name, iface.addr.ip());
        }
    }

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
                log::info!("RESPONDER RECEIVED: {} bytes from {:?}", data.len(), src_addr);
                error_count.store(0, Ordering::Relaxed);
                match Apdu::decode(&data) {
                    Ok(apdu) => {
                        match apdu {
                            Apdu::UnconfirmedRequest { service_choice, service_data } => {
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
                                            
                                            let mut service_data = Vec::new();
                                            iam.encode(&mut service_data).map_err(|e| anyhow::anyhow!(e.to_string()))?;

                                            let apdu = Apdu::UnconfirmedRequest {
                                                service_choice: UnconfirmedServiceChoice::IAm as u8,
                                                service_data,
                                            };
                                            
                                            let encoded = apdu.encode();
                                            
                                            // Send broadcast
                                            if let Err(e) = datalink.send_frame(&encoded, &bacnet_rs::datalink::DataLinkAddress::Broadcast) {
                                                log::error!("Failed to send I-Am broadcast: {}", e);
                                            } else {
                                                log::info!("Sent I-Am broadcast ({} bytes)", encoded.len());
                                            }

                                            // Also send directed back to requester for better reliability
                                            let _ = datalink.send_frame(&encoded, &src_addr);
                                        }
                                    }
                                }
                            }
                            Apdu::ConfirmedRequest { invoke_id, service_choice, service_data, .. } => {
                                if service_choice == 12 { // ReadProperty
                                    let s = state.lock().unwrap();
                                    match decode_read_property_request(&service_data) {
                                        Ok(req) => {
                                            log::info!("Received ReadProperty: {:?} for property {}", req.object_identifier, req.property_identifier);
                                            
                                            let result = if req.property_identifier == PropertyIdentifier::ObjectList as u32 {
                                                let list = s.db.get_all_objects();
                                                log::info!("Responding with ObjectList ({} objects)", list.len());
                                                let val = PropertyValue::Array(list.into_iter().map(PropertyValue::ObjectIdentifier).collect());
                                                Some(val)
                                            } else {
                                                s.db.get_property(req.object_identifier, unsafe { std::mem::transmute(req.property_identifier) }).ok()
                                            };

                                            if let Some(val) = result {
                                                let mut response_data = Vec::new();
                                                if let Err(e) = encode_read_property_response(&mut response_data, req.object_identifier, req.property_identifier, val) {
                                                    log::error!("Failed to encode ReadProperty response: {}", e);
                                                } else {
                                                    let ack = Apdu::ComplexAck {
                                                        segmented: false,
                                                        more_follows: false,
                                                        invoke_id,
                                                        sequence_number: None,
                                                        proposed_window_size: None,
                                                        service_choice,
                                                        service_data: response_data,
                                                    };
                                                    if let Err(e) = datalink.send_frame(&ack.encode(), &src_addr) {
                                                        log::error!("Failed to send ComplexAck: {}", e);
                                                    } else {
                                                        log::info!("Sent ComplexAck to {:?}", src_addr);
                                                    }
                                                }
                                            } else {
                                                log::warn!("Property {} not found on object {:?}", req.property_identifier, req.object_identifier);
                                                let err = Apdu::Error {
                                                    invoke_id,
                                                    service_choice,
                                                    error_class: 1, // Object
                                                    error_code: 31, // Unknown property
                                                };
                                                let _ = datalink.send_frame(&err.encode(), &src_addr);
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("Failed to decode ReadProperty request: {}", e);
                                        }
                                    }
                                } else if service_choice == 15 { // WriteProperty
                                    let mut s = state.lock().unwrap();
                                    match decode_write_property_request(&service_data) {
                                        Ok((obj_id, prop_id, val)) => {
                                            log::info!("Received WriteProperty: {:?} property {} = {:?}", obj_id, prop_id, val);
                                            if let Err(e) = s.db.set_property(obj_id, unsafe { std::mem::transmute(prop_id) }, val) {
                                                log::error!("Failed to set property: {}", e);
                                                let err = Apdu::Error {
                                                    invoke_id,
                                                    service_choice,
                                                    error_class: 1, // Object
                                                    error_code: 31, // Unknown property or failed write
                                                };
                                                let _ = datalink.send_frame(&err.encode(), &src_addr);
                                            } else {
                                                log::info!("Property set successfully. Sending SimpleAck");
                                                let ack = Apdu::SimpleAck {
                                                    invoke_id,
                                                    service_choice,
                                                };
                                                let _ = datalink.send_frame(&ack.encode(), &src_addr);
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("Failed to decode WriteProperty request: {}", e);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to decode APDU: {}", e);
                    }
                }
            }
            Err(e) => {
                let err_str = e.to_string();
                if !err_str.contains("TimedOut") && !err_str.contains("WouldBlock") && !err_str.contains("Resource temporarily unavailable") {
                    let count = error_count.fetch_add(1, Ordering::Relaxed);
                    if count == 0 || count % 100 == 0 {
                        log::error!("Datalink error (count: {}): {}", count + 1, e);
                    }
                }
                sleep(Duration::from_millis(100)).await;
            }
        }
        tokio::task::yield_now().await;
    }
}

fn decode_read_property_request(data: &[u8]) -> anyhow::Result<ReadPropertyRequest> {
    let mut pos = 0;
    let ((obj_type, instance), c1) = encoding::decode_context_object_id(&data[pos..], 0)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    pos += c1;
    let (prop_id, _c2) = encoding::decode_context_enumerated(&data[pos..], 1)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    
    Ok(ReadPropertyRequest::new(
        ObjectIdentifier::new(ObjectType::try_from(obj_type).unwrap(), instance),
        prop_id,
    ))
}

fn decode_write_property_request(data: &[u8]) -> anyhow::Result<(ObjectIdentifier, u32, PropertyValue)> {
    let mut pos = 0;
    // 1. Object ID (Context 0)
    let ((obj_type, instance), c1) = encoding::decode_context_object_id(&data[pos..], 0)
        .map_err(|e| anyhow::anyhow!("Failed to decode object id: {}", e))?;
    pos += c1;
    
    // 2. Property ID (Context 1)
    let (prop_id, c2) = encoding::decode_context_enumerated(&data[pos..], 1)
        .map_err(|e| anyhow::anyhow!("Failed to decode property id: {}", e))?;
    pos += c2;

    // 3. Optional Array Index (Context 2) - skip if present
    if pos < data.len() && (data[pos] & 0xF8) == 0x20 {
        let (_, c3) = encoding::decode_context_unsigned(&data[pos..], 2)
            .map_err(|e| anyhow::anyhow!("Failed to decode array index: {}", e))?;
        pos += c3;
    }

    // 4. Property Value (Context 3 - Opening Tag 3)
    if pos >= data.len() || data[pos] != 0x3E {
        anyhow::bail!("Expected opening tag for property value (0x3E) at pos {}, got {:?}", pos, data.get(pos));
    }
    pos += 1;

    let (val, consumed) = decode_property_value(&data[pos..])?;
    pos += consumed;

    // Closing Tag 3
    if pos >= data.len() || data[pos] != 0x3F {
        anyhow::bail!("Expected closing tag for property value (0x3F) at pos {}, got {:?}", pos, data.get(pos));
    }
    
    Ok((
        ObjectIdentifier::new(ObjectType::try_from(obj_type).unwrap(), instance),
        prop_id,
        val
    ))
}

fn decode_property_value(data: &[u8]) -> anyhow::Result<(PropertyValue, usize)> {
    // This is a simplified decoder for common types
    if data.is_empty() {
        anyhow::bail!("Empty data for property value");
    }

    let tag = data[0];
    if (tag & 0x08) == 0 { // Application Tag
        let app_tag = tag >> 4;
        match app_tag {
            1 => { // Boolean
                Ok((PropertyValue::Boolean(tag == 0x11), 1))
            }
            2 => { // Unsigned
                let (v, c) = encoding::decode_unsigned(data)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                Ok((PropertyValue::UnsignedInteger(v), c))
            }
            4 => { // Real
                let (v, c) = encoding::decode_real(data)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                Ok((PropertyValue::Real(v), c))
            }
            9 => { // Enumerated
                let (v, c) = encoding::decode_enumerated(data)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                Ok((PropertyValue::Enumerated(v), c))
            }
            12 => { // Object Identifier
                let ((t, i), c) = encoding::decode_object_identifier(data)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                Ok((PropertyValue::ObjectIdentifier(ObjectIdentifier::new(ObjectType::try_from(t).unwrap(), i)), c))
            }
            _ => anyhow::bail!("Unsupported application tag: {}", app_tag),
        }
    } else {
        anyhow::bail!("Context tags inside property value not supported in this simplified decoder")
    }
}

fn encode_read_property_response(buf: &mut Vec<u8>, obj_id: ObjectIdentifier, prop_id: u32, val: PropertyValue) -> anyhow::Result<()> {
    // 1. Object ID (Context 0)
    buf.extend(encode_context_object_id(obj_id.object_type as u16, obj_id.instance, 0).map_err(|e| anyhow::anyhow!(e.to_string()))?);
    // 2. Property ID (Context 1)
    buf.extend(encode_context_enumerated(prop_id, 1).map_err(|e| anyhow::anyhow!(e.to_string()))?);
    // 3. Value (Context 3)
    buf.push(0x3E); // Opening Tag 3
    encode_property_value(buf, val)?;
    buf.push(0x3F); // Closing Tag 3
    Ok(())
}

fn encode_property_value(buf: &mut Vec<u8>, val: PropertyValue) -> anyhow::Result<()> {
    match val {
        PropertyValue::Real(f) => encoding::encode_real(buf, f).map_err(|e| anyhow::anyhow!(e.to_string()))?,
        PropertyValue::Boolean(b) => encoding::encode_boolean(buf, b).map_err(|e| anyhow::anyhow!(e.to_string()))?,
        PropertyValue::UnsignedInteger(u) => encoding::encode_unsigned(buf, u).map_err(|e| anyhow::anyhow!(e.to_string()))?,
        PropertyValue::CharacterString(s) => encoding::encode_character_string(buf, &s).map_err(|e| anyhow::anyhow!(e.to_string()))?,
        PropertyValue::ObjectIdentifier(id) => encoding::encode_object_identifier(buf, id.object_type as u16, id.instance).map_err(|e| anyhow::anyhow!(e.to_string()))?,
        PropertyValue::Enumerated(e) => encoding::encode_enumerated(buf, e).map_err(|e| anyhow::anyhow!(e.to_string()))?,
        PropertyValue::Array(arr) => {
            for v in arr {
                encode_property_value(buf, v)?;
            }
        }
        _ => anyhow::bail!("Unsupported property value type for encoding"),
    }
    Ok(())
}
