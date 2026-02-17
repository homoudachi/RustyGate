use crate::common::types::{Command, Event};
pub mod network;
pub mod bacnet;
use crate::core::network::interface;
use crate::core::bacnet::client::BacnetClient;
use crate::core::bacnet::discovery;
use tokio::sync::{mpsc, broadcast};
use anyhow::Result;
use bacnet_rs::app::Apdu;
use std::sync::{Arc, Mutex};

pub struct Core {
    cmd_rx: mpsc::Receiver<Command>,
    event_tx: broadcast::Sender<Event>,
    bacnet_client: Option<Arc<Mutex<BacnetClient>>>,
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
        tokio::task::spawn_blocking(move || {
            log::info!("BACnet receiver thread started for {}", iface.ip);
            loop {
                let mut client_lock = client_arc.lock().unwrap();
                // receive_frame has internal 100ms timeout
                if let Ok(Some((data, src))) = client_lock.receive_frame() {
                    if let Ok(apdu) = Apdu::decode(&data) {
                        if let Ok(Some(mut device)) = discovery::parse_i_am(&apdu) {
                            device.address = format!("{:?}", src);
                            log::info!("Discovered device: {:?} from {:?}", device, src);
                            let _ = event_tx.send(Event::DeviceDiscovered(device));
                        }
                    }
                }
                std::thread::yield_now();
            }
        });

        self.event_tx.send(Event::StatusMessage(format!("Bound to {}", iface.ip)))?;
        log::info!("Bound to {}", iface.ip);
        Ok(())
    }

    async fn start_discovery(&mut self) -> Result<()> {
        if let Some(client_mutex) = &self.bacnet_client {
            let mut client = client_mutex.lock().unwrap();
            client.send_who_is(None, None)?;
            self.event_tx.send(Event::StatusMessage("Who-Is broadcast sent".to_string()))?;
        } else {
            self.event_tx.send(Event::StatusMessage("Error: No interface bound".to_string()))?;
        }
        Ok(())
    }
}
