use bacnet_rs::{
    app::{Apdu, MaxApduSize, MaxSegments},
    datalink::{bip::BacnetIpDataLink, DataLink, DataLinkAddress},
    service::{UnconfirmedServiceChoice, WhoIsRequest, ReadPropertyRequest},
};
use anyhow::Result;
use std::net::SocketAddr;

pub struct BacnetClient {
    pub datalink: BacnetIpDataLink,
    invoke_id: u8,
}

impl BacnetClient {
    pub fn new(bind_addr: SocketAddr) -> Result<Self> {
        let datalink = BacnetIpDataLink::new(bind_addr)?;
        Ok(Self { datalink, invoke_id: 0 })
    }

    fn next_invoke_id(&mut self) -> u8 {
        let id = self.invoke_id;
        self.invoke_id = self.invoke_id.wrapping_add(1);
        id
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
        log::info!("Sending Who-Is from {:?} to {:?}", self.datalink.local_address(), dest);
        self.datalink.send_frame(&encoded, &dest)?;
        
        Ok(())
    }

    pub fn send_read_property(&mut self, dest: &DataLinkAddress, obj_id: bacnet_rs::object::ObjectIdentifier, prop_id: u32) -> Result<u8> {
        let req = ReadPropertyRequest::new(obj_id, prop_id);
        let mut data = Vec::new();
        req.encode(&mut data).map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let invoke_id = self.next_invoke_id();
        let apdu = Apdu::ConfirmedRequest {
            segmented: false,
            more_follows: false,
            segmented_response_accepted: true,
            max_segments: MaxSegments::Unspecified,
            max_response_size: MaxApduSize::Up1476,
            invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: 12, // ReadProperty
            service_data: data,
        };

        let encoded = apdu.encode();
        self.datalink.send_frame(&encoded, dest)?;
        
        Ok(invoke_id)
    }
}
