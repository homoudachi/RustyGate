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
    DiscoverObjects {
        interface: String,
        device_id: u32,
        address: String,
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
    DeviceObjectsDiscovered {
        device_id: u32,
        objects: Vec<BacnetObjectInfo>,
    },
    PointUpdate {
        device_id: u32,
        object_id: String,
        value: f32,
    },
    StatusMessage(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacnetObjectInfo {
    pub object_type: u16,
    pub instance: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacnetDevice {
    pub instance: u32,
    pub address: String,
    pub name: String,
}
