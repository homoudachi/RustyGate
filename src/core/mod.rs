use crate::common::types::{Command, Event};
pub mod network;
pub mod bacnet;
use crate::core::network::interface;
use crate::core::bacnet::client::BacnetClient;
use tokio::sync::{mpsc, broadcast};
use anyhow::Result;

pub struct Core {
    cmd_rx: mpsc::Receiver<Command>,
    event_tx: broadcast::Sender<Event>,
}

impl Core {
    pub fn new(cmd_rx: mpsc::Receiver<Command>, event_tx: broadcast::Sender<Event>) -> Self {
        Self { cmd_rx, event_tx }
    }

    pub async fn run(&mut self) -> Result<()> {
        log::info!("Starting Core Engine...");

        loop {
            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => {
                    self.handle_command(cmd).await?;
                }
                // Here we will later add MQTT and BACnet event loop polling
            }
        }
    }

    async fn handle_command(&self, cmd: Command) -> Result<()> {
        log::info!("Core received command: {:?}", cmd);
        match cmd {
            Command::StartDiscovery => {
                self.event_tx.send(Event::StatusMessage("Listing network interfaces...".to_string()))?;
                match interface::list_interfaces() {
                    Ok(ifaces) => {
                        if let Some(iface) = ifaces.first() {
                            log::info!("Binding to first available interface: {} ({})", iface.name, iface.ip);
                            let addr = std::net::SocketAddr::new(iface.ip, 47808);
                            match BacnetClient::new(addr) {
                                Ok(client) => {
                                    let _ = client.send_who_is(None, None);
                                }
                                Err(e) => log::error!("Failed to initialize BACnet client: {}", e),
                            }
                        }
                    }
                    Err(e) => log::error!("Failed to list interfaces: {}", e),
                }
                self.event_tx.send(Event::StatusMessage("Starting BACnet discovery...".to_string()))?;
            }
            _ => {
                log::warn!("Command not yet implemented: {:?}", cmd);
            }
        }
        Ok(())
    }
}
