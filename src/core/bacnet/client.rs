use bacnet_rs::{
    app::Apdu,
    datalink::{bip::BacnetIpDataLink, DataLink, DataLinkAddress},
    service::{UnconfirmedServiceChoice, WhoIsRequest},
};
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

    pub fn send_who_is(&mut self, low: Option<u32>, high: Option<u32>, destination: Option<DataLinkAddress>) -> Result<()> {
        let mut who_is = WhoIsRequest::new();
        who_is.device_instance_range_low_limit = low;
        who_is.device_instance_range_high_limit = high;
        
        let mut data = Vec::new();
        who_is.encode(&mut data).map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let apdu = Apdu::UnconfirmedRequest {
            service_choice: UnconfirmedServiceChoice::WhoIs as u8,
            service_data: data,
        };

        let encoded = apdu.encode();
        let dest = destination.unwrap_or(DataLinkAddress::Broadcast);
        self.datalink.send_frame(&encoded, &dest)?;
        
        log::info!("Sent Who-Is request to {:?}", dest);
        Ok(())
    }

    pub fn receive_frame(&mut self) -> Result<Option<(Vec<u8>, DataLinkAddress)>> {
        match self.datalink.receive_frame() {
            Ok((data, src)) => Ok(Some((data, src))),
            Err(e) if e.to_string().contains("TimedOut") || e.to_string().contains("WouldBlock") => Ok(None),
            Err(e) => Err(anyhow::anyhow!(e.to_string())),
        }
    }
}
