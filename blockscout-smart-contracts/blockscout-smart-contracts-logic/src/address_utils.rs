use alloy_primitives::Address;
use std::str::FromStr;
use tonic::Status;

pub fn parse_address_to_bytes(s: &str) -> Result<Vec<u8>, Status> {
    Address::from_str(s)
        .map(|a| a.to_vec())
        .map_err(|e| Status::invalid_argument(format!("invalid contract.address: {e}")))
}

pub fn format_address_hex_from_db(address_db: &[u8], fallback: &str) -> String {
    if address_db.len() == 20 {
        let a = Address::from_slice(address_db);
        format!("{a:#x}")
    } else {
        fallback.to_string()
    }
}
