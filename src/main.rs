mod core;
mod ui;
mod common;

use crate::core::Core;
use crate::common::types::{Command, Event};
use crate::core::network::interface;
use tokio::sync::{mpsc, broadcast};
use std::env;

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();

    // Check for CLI mode
    if args.len() > 1 {
        match args[1].as_str() {
            "list" => {
                println!("Available Network Interfaces:");
                if let Ok(ifaces) = interface::list_interfaces() {
                    for iface in ifaces {
                        println!(" - {}", iface.name);
                    }
                }
                return;
            }
            "discover" => {
                if let Some(iface_name) = args.get(2) {
                    println!("Running manual discovery on {}...", iface_name);
                    run_core_oneshot(Command::BindAndDiscover(iface_name.clone()));
                    return;
                } else {
                    println!("Usage: cargo run -- discover <interface_name>");
                    return;
                }
            }
            "ping" => {
                if let (Some(iface), Some(target)) = (args.get(2), args.get(3)) {
                    println!("Pinging {} via {}...", target, iface);
                    run_core_oneshot(Command::Ping { 
                        interface: iface.clone(), 
                        target: target.clone() 
                    });
                    return;
                } else {
                    println!("Usage: cargo run -- ping <interface_name> <target_ip>");
                    return;
                }
            }
            "discover-objects" => {
                if let (Some(iface), Some(device_id), Some(address)) = (args.get(2), args.get(3), args.get(4)) {
                    let id = device_id.parse().unwrap();
                    println!("Discovering objects on {} ({}) via {}...", id, address, iface);
                    run_core_oneshot(Command::DiscoverObjects { 
                        interface: iface.clone(),
                        device_id: id,
                        address: address.clone()
                    });
                    return;
                } else {
                    println!("Usage: cargo run -- discover-objects <interface_name> <device_id> <device_address>");
                    return;
                }
            }
            _ => {} // Fall through to standard app launch
        }
    }

    // Standard Launch (Core + Web UI)
    let with_simulator = args.iter().any(|arg| arg == "--with-simulator");
    
    let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(100);
    let (event_tx, mut _event_rx) = broadcast::channel(100);

    let core_event_tx = event_tx.clone();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let cmd_tx_clone = cmd_tx.clone();
        let event_tx_clone = event_tx.clone();
        
        if with_simulator {
            log::info!("Starting local BACnet simulator...");
            tokio::spawn(async move {
                let status = tokio::process::Command::new("cargo")
                    .arg("run")
                    .arg("--quiet")
                    .arg("--")
                    .arg("99999")
                    .arg("localhost")
                    .arg("lo")
                    .current_dir("tests/bacnet-responder")
                    .status()
                    .await;
                if let Err(e) = status {
                    log::error!("Failed to start simulator: {}", e);
                }
            });
            // Give simulator a moment to bind
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }

        tokio::spawn(async move {
            ui::launch(cmd_tx_clone, event_tx_clone).await;
        });

        let mut core = Core::new(cmd_rx, core_event_tx);
        let shutdown_trigger = core.shutdown.clone();
        
        // Spawn core in background
        tokio::spawn(async move {
            if let Err(e) = core.run().await {
                log::error!("Core engine error: {}", e);
            }
        });

        log::info!("RustyGate is running. Press Ctrl+C to stop.");
        let _ = tokio::signal::ctrl_c().await;
        log::info!("Shutting down...");
        shutdown_trigger.store(true, std::sync::atomic::Ordering::SeqCst);
        
        // Give it a moment to stop threads
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    });
}

fn run_core_oneshot(cmd: Command) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(100);
    let (event_tx, mut event_rx) = broadcast::channel(100);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut core = Core::new(cmd_rx, event_tx);
        
        let _ = cmd_tx.send(cmd).await;

        let core_shutdown_handle = core.shutdown.clone();
        tokio::spawn(async move {
            if let Err(e) = core.run().await {
                log::error!("Core error: {}", e);
            }
        });

        println!("Waiting for devices (5s)...");
        let timeout = tokio::time::sleep(std::time::Duration::from_secs(5));
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                Ok(event) = event_rx.recv() => {
                    match event {
                        Event::DeviceDiscovered(dev) => {
                            println!("FOUND DEVICE: ID={} Address={}", dev.instance, dev.address);
                        }
                        Event::DeviceObjectsDiscovered { device_id, objects } => {
                            println!("OBJECTS DISCOVERED for Device {}:", device_id);
                            for obj in objects {
                                println!(" - [{:?}] {} (Instance {})", obj.object_type, obj.name, obj.instance);
                            }
                        }
                        Event::StatusMessage(msg) => println!("Status: {}", msg),
                        _ => {}
                    }
                }
                _ = &mut timeout => {
                    println!("Discovery timed out.");
                    core_shutdown_handle.store(true, std::sync::atomic::Ordering::SeqCst);
                    break;
                }
            }
        }
        // Give it a moment to clean up
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    });
}
