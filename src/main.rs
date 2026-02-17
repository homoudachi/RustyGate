mod core;
mod ui;
mod common;

use crate::core::Core;
use crate::common::types::Command;
use tokio::sync::{mpsc, broadcast};

fn main() {
    // Initialize logging
    env_logger::init();

    // Create channels for Inter-Task Communication
    // UI -> Core
    let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(100);
    // Core -> UI
    let (event_tx, _event_rx) = broadcast::channel(100);

    // Start the Tokio runtime for both Core and Web UI
    let core_event_tx = event_tx.clone();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let cmd_tx_clone = cmd_tx.clone();
        let event_tx_clone = event_tx.clone();
        
        // Spawn the Web UI
        tokio::spawn(async move {
            ui::launch(cmd_tx_clone, event_tx_clone).await;
        });

        let mut core = Core::new(cmd_rx, core_event_tx);
        if let Err(e) = core.run().await {
            log::error!("Core engine error: {}", e);
        }
    });
}
