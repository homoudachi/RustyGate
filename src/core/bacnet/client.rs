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

    pub fn send_write_property(&mut self, dest: &DataLinkAddress, obj_id: bacnet_rs::object::ObjectIdentifier, prop_id: u32, value: bacnet_rs::object::PropertyValue) -> Result<u8> {
        use bacnet_rs::encoding::*;

        let mut data = Vec::new();
        // 1. Object ID (Context 0)
        data.extend(encode_context_object_id(obj_id.object_type as u16, obj_id.instance, 0).map_err(|e| anyhow::anyhow!(e.to_string()))?);
        // 2. Property ID (Context 1)
        data.extend(encode_context_enumerated(prop_id, 1).map_err(|e| anyhow::anyhow!(e.to_string()))?);
        // 3. Value (Context 3)
        data.push(0x3E); // Opening Tag 3
        match &value {
            bacnet_rs::object::PropertyValue::Real(f) => encode_real(&mut data, *f).map_err(|e| anyhow::anyhow!(e.to_string()))?,
            bacnet_rs::object::PropertyValue::Boolean(b) => encode_boolean(&mut data, *b).map_err(|e| anyhow::anyhow!(e.to_string()))?,
            _ => anyhow::bail!("Unsupported value type for WriteProperty"),
        }
        data.push(0x3F); // Closing Tag 3

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
            service_choice: 15, // WriteProperty
            service_data: data,
        };

        let encoded = apdu.encode();
        self.datalink.send_frame(&encoded, dest)?;
        
        Ok(invoke_id)
    }
}
