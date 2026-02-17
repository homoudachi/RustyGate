use crate::common::types::{Command, Event};
pub mod network;
pub mod bacnet;
use crate::core::network::interface;
use crate::core::bacnet::client::BacnetClient;
use crate::core::bacnet::discovery;
use tokio::sync::{mpsc, broadcast};
use anyhow::Result;
use bacnet_rs::app::Apdu;

pub struct Core {
    cmd_rx: mpsc::Receiver<Command>,
    event_tx: broadcast::Sender<Event>,
    bacnet_client: Option<BacnetClient>,
}

impl Core {
    pub fn new(cmd_rx: mpsc::Receiver<Command>, event_tx: broadcast::Sender<Event>) -> Self {
        Self { 
            cmd_rx, 
            event_tx,
            bacnet_client: None,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        log::info!("Starting Core Engine...");

        loop {
            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => {
                    self.handle_command(cmd).await?;
                }
                _ = async {
                    if let Some(client) = &mut self.bacnet_client {
                        if let Ok(Some((data, src))) = client.receive_frame() {
                            if let Ok(apdu) = Apdu::decode(&data) {
                                if let Ok(Some(mut device)) = discovery::parse_i_am(&apdu) {
                                    device.address = format!("{:?}", src);
                                    log::info!("Discovered device: {:?} from {:?}", device, src);
                                    let _ = self.event_tx.send(Event::DeviceDiscovered(device));
                                }
                            }
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                } => {}
            }
        }
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
            _ => {
                log::warn!("Command not yet implemented: {:?}", cmd);
            }
        }
        Ok(())
    }

    async fn bind_interface(&mut self, name: &str) -> Result<()> {
        self.event_tx.send(Event::StatusMessage(format!("Binding to {}...", name)))?;
        if let Ok(ifaces) = interface::list_interfaces() {
            if let Some(iface) = ifaces.into_iter().find(|i| i.name == name) {
                let addr = std::net::SocketAddr::new(iface.ip, 47808);
                match BacnetClient::new(addr) {
                    Ok(client) => {
                        self.bacnet_client = Some(client);
                        self.event_tx.send(Event::StatusMessage(format!("Bound to {}", iface.ip)))?;
                        log::info!("Bound to {}", iface.ip);
                    }
                    Err(e) => log::error!("Failed to initialize BACnet client: {}", e),
                }
            }
        }
        Ok(())
    }

    async fn start_discovery(&mut self) -> Result<()> {
        if let Some(client) = &mut self.bacnet_client {
            client.send_who_is(None, None)?;
            self.event_tx.send(Event::StatusMessage("Who-Is broadcast sent".to_string()))?;
        } else {
            self.event_tx.send(Event::StatusMessage("Error: No interface bound".to_string()))?;
        }
        Ok(())
    }
}
