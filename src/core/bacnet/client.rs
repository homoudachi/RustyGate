use bacnet_rs::datalink::bip::BacnetIpDataLink;
use bacnet_rs::service::WhoIsRequest;
use anyhow::Result;
use std::net::SocketAddr;

pub struct BacnetClient {
    datalink: BacnetIpDataLink,
}

impl BacnetClient {
    pub fn new(bind_addr: SocketAddr) -> Result<Self> {
        let datalink = BacnetIpDataLink::new(bind_addr)?;
        Ok(Self { datalink })
    }

    pub fn send_who_is(&self, low: Option<u32>, high: Option<u32>) -> Result<()> {
        let mut who_is = WhoIsRequest::new();
        who_is.device_instance_range_low_limit = low;
        who_is.device_instance_range_high_limit = high;
        
        log::info!("Sending Who-Is request: {:?}", who_is);
        Ok(())
    }
}
