use crate::common::types::{Command, Event};
pub mod network;
pub mod bacnet;
use crate::core::network::interface;
use crate::core::bacnet::client::BacnetClient;
use crate::core::bacnet::discovery;
use tokio::sync::{mpsc, broadcast};
use anyhow::Result;
use bacnet_rs::app::Apdu;
use bacnet_rs::object::{PropertyIdentifier, ObjectIdentifier, ObjectType};
use std::sync::{Arc, Mutex};

pub struct Core {
    cmd_rx: mpsc::Receiver<Command>,
    event_tx: broadcast::Sender<Event>,
    bacnet_client: Option<Arc<Mutex<BacnetClient>>>,
    pub shutdown: Arc<std::sync::atomic::AtomicBool>,
}

impl Core {
    pub fn new(cmd_rx: mpsc::Receiver<Command>, event_tx: broadcast::Sender<Event>) -> Self {
        Self { 
            cmd_rx, 
            event_tx,
            bacnet_client: None,
            shutdown: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        log::info!("Starting Core Engine...");

        loop {
            if self.shutdown.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => {
                    self.handle_command(cmd).await?;
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                    // Check shutdown occasionally
                }
            }
        }
        log::info!("Core Engine shut down.");
        Ok(())
    }

    async fn handle_command(&mut self, cmd: Command) -> Result<()> {
        log::info!("Core received command: {:?}", cmd);
        match cmd {
            Command::BindAndDiscover(name) => {
                self.bind_interface(&name).await?;
                self.start_discovery().await?;
            }
            Command::BindInterface(name) => {
                self.bind_interface(&name).await?;
            }
            Command::StartDiscovery => {
                self.start_discovery().await?;
            }
            Command::Ping { interface, target } => {
                self.bind_interface(&interface).await?;
                if let Some(client_mutex) = &self.bacnet_client {
                    let mut client = client_mutex.lock().unwrap();
                    let target_addr: std::net::IpAddr = target.parse()?;
                    let dest = bacnet_rs::datalink::DataLinkAddress::Ip(
                        std::net::SocketAddr::new(target_addr, 47808)
                    );
                    client.send_who_is(None, None, Some(dest))?;
                    self.event_tx.send(Event::StatusMessage(format!("Sent targeted Who-Is to {}", target)))?;
                }
            }
            Command::DiscoverObjects { interface, device_id, address } => {
                log::info!("Handling DiscoverObjects for device {} at {} via {}", device_id, address, interface);
                self.bind_interface(&interface).await?;
                if let Some(client_mutex) = &self.bacnet_client {
                    let mut client = client_mutex.lock().unwrap();
                    match address.parse::<std::net::SocketAddr>() {
                        Ok(target_addr) => {
                            let dest = bacnet_rs::datalink::DataLinkAddress::Ip(target_addr);
                            let obj_id = ObjectIdentifier::new(ObjectType::Device, device_id);
                            match client.send_read_property(&dest, obj_id, PropertyIdentifier::ObjectList as u32) {
                                Ok(id) => log::info!("Sent ReadProperty(ObjectList) to {}, invoke_id={}", address, id),
                                Err(e) => log::error!("Failed to send ReadProperty: {}", e),
                            }
                            self.event_tx.send(Event::StatusMessage(format!("Requested object list from device {}", device_id)))?;
                        }
                        Err(e) => log::error!("Failed to parse address {}: {}", address, e),
                    }
                }
            }
            _ => {
                log::warn!("Command not yet implemented: {:?}", cmd);
            }
        }
        Ok(())
    }

    async fn bind_interface(&mut self, name: &str) -> Result<()> {
        self.event_tx.send(Event::StatusMessage(format!("Binding to {}...", name)))?;
        
        let iface = interface::list_interfaces()?
            .into_iter()
            .find(|i| i.name == name)
            .ok_or_else(|| anyhow::anyhow!("Interface not found"))?;

        let addr = std::net::SocketAddr::new(iface.ip, 47808);
        let client = BacnetClient::new(addr)?;
        let client_arc = Arc::new(Mutex::new(client));
        self.bacnet_client = Some(Arc::clone(&client_arc));

        // Spawn a dedicated thread for receiving frames (blocking I/O)
        let event_tx = self.event_tx.clone();
        let shutdown = Arc::clone(&self.shutdown);
        tokio::task::spawn_blocking(move || {
            log::info!("BACnet receiver thread started for {}", iface.ip);
            let mut last_error_log = std::time::Instant::now() - std::time::Duration::from_secs(60);
            let mut error_count = 0;

            loop {
                if shutdown.load(std::sync::atomic::Ordering::SeqCst) {
                    log::info!("BACnet receiver thread shutting down.");
                    break;
                }
                let mut client_lock = client_arc.lock().unwrap();
                match client_lock.receive_frame() {
                    Ok(Some((data, src))) => {
                        error_count = 0;
                        if let Ok(apdu) = Apdu::decode(&data) {
                            match apdu {
                                Apdu::UnconfirmedRequest { .. } => {
                                    if let Ok(Some(mut device)) = discovery::parse_i_am(&apdu) {
                                        device.address = format!("{:?}", src);
                                        log::info!("Discovered device: {:?} from {:?}", device, src);
                                        let _ = event_tx.send(Event::DeviceDiscovered(device));
                                    }
                                }
                                Apdu::ComplexAck { .. } => {
                                    if let Ok(Some(resp)) = discovery::parse_read_property_response(&apdu) {
                                        if resp.property_identifier == PropertyIdentifier::ObjectList as u32 {
                                            if let Ok(objects) = discovery::parse_object_list(&resp.property_value) {
                                                log::info!("Discovered {} objects on device {}", objects.len(), resp.object_identifier.instance);
                                                let _ = event_tx.send(Event::DeviceObjectsDiscovered {
                                                    device_id: resp.object_identifier.instance,
                                                    objects: objects.into_iter().map(|id| crate::common::types::BacnetObjectInfo {
                                                        object_type: id.object_type as u16,
                                                        instance: id.instance,
                                                        name: format!("{:?} {}", id.object_type, id.instance),
                                                    }).collect(),
                                                });
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(None) => {
                        // Timeout/WouldBlock - no data
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Err(e) => {
                        let err_str = e.to_string();
                        if err_str.contains("WouldBlock") || err_str.contains("Resource temporarily unavailable") {
                            // Equivalent to a timeout, just sleep and continue
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        } else {
                            error_count += 1;
                            if last_error_log.elapsed() > std::time::Duration::from_secs(10) {
                                log::error!("BACnet receiver error (count: {}): {}", error_count, e);
                                last_error_log = std::time::Instant::now();
                            }
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                    }
                }
            }
        });

        self.event_tx.send(Event::StatusMessage(format!("Bound to {}", iface.ip)))?;
        log::info!("Bound to {}", iface.ip);
        Ok(())
    }

    async fn start_discovery(&mut self) -> Result<()> {
        if let Some(client_mutex) = &self.bacnet_client {
            let mut client = client_mutex.lock().unwrap();
            client.send_who_is(None, None, None)?;
            self.event_tx.send(Event::StatusMessage("Who-Is broadcast sent".to_string()))?;
        } else {
            self.event_tx.send(Event::StatusMessage("Error: No interface bound".to_string()))?;
        }
        Ok(())
    }
}
