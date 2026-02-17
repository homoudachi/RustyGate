use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    StartDiscovery,
    StopDiscovery,
    BindInterface(String),
    /// For CLI testing: binds and starts discovery immediately
    BindAndDiscover(String),
    /// Send a Who-Is to a specific IP
    Ping {
        interface: String,
        target: String,
    },
    WriteProperty {
        device_id: u32,
        object_type: u16,
        instance: u32,
        property: u32,
        value: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    DeviceDiscovered(BacnetDevice),
    PointUpdate {
        device_id: u32,
        object_id: String,
        value: f32,
    },
    StatusMessage(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacnetDevice {
    pub instance: u32,
    pub address: String,
    pub name: String,
}
