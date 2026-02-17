use bacnet_rs::{
    app::Apdu,
    service::{IAmRequest, UnconfirmedServiceChoice, ReadPropertyResponse},
    object::{ObjectType, ObjectIdentifier},
    encoding,
};
use crate::common::types::BacnetDevice;
use anyhow::Result;

/// Parses an I-Am response and returns a BacnetDevice if successful.
pub fn parse_i_am(apdu: &Apdu) -> Result<Option<BacnetDevice>> {
    if let Apdu::UnconfirmedRequest { service_choice, service_data } = apdu {
        if *service_choice == UnconfirmedServiceChoice::IAm as u8 {
            let i_am = IAmRequest::decode(service_data)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            
            return Ok(Some(BacnetDevice {
                instance: i_am.device_identifier.instance,
                address: "Unknown".to_string(), // In real usage, we'd get this from the DataLink source
                name: format!("Device {}", i_am.device_identifier.instance),
            }));
        }
    }
    Ok(None)
}

pub fn parse_read_property_response(apdu: &Apdu) -> Result<Option<ReadPropertyResponse>> {
    if let Apdu::ComplexAck { service_choice, service_data, .. } = apdu {
        if *service_choice == 12 { // ReadProperty
            let resp = ReadPropertyResponse::decode(service_data)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            return Ok(Some(resp));
        }
    }
    Ok(None)
}

pub fn parse_object_list(data: &[u8]) -> Result<Vec<ObjectIdentifier>> {
    let mut objects = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        if let Ok(((obj_type, instance), consumed)) = encoding::decode_object_identifier(&data[pos..]) {
            objects.push(ObjectIdentifier::new(ObjectType::try_from(obj_type).unwrap(), instance));
            pos += consumed;
        } else {
            break;
        }
    }
    Ok(objects)
}
