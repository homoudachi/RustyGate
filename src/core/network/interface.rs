use anyhow::Result;
use if_addrs::get_if_addrs;
use std::net::IpAddr;

#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub ip: IpAddr,
}

/// Discovers all available network interfaces with IPv4 addresses.
pub fn list_interfaces() -> Result<Vec<NetworkInterface>> {
    let addrs = get_if_addrs()?;
    let interfaces = addrs
        .into_iter()
        .filter(|iface| !iface.is_loopback() && iface.addr.ip().is_ipv4())
        .map(|iface| NetworkInterface {
            name: iface.name,
            ip: iface.addr.ip(),
        })
        .collect();

    Ok(interfaces)
}
