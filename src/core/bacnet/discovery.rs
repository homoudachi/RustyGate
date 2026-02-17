use bacnet_rs::{
    app::Apdu,
    service::{IAmRequest, UnconfirmedServiceChoice},
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
