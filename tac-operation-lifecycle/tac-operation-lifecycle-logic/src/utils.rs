use anyhow::anyhow;
use std::str::FromStr;
//use base64::prelude::*;
use tonlib_core::TonAddress;

pub fn is_generic_hash(q: &str) -> bool {
    q.starts_with("0x") && q.len() == 66 && q[2..].chars().all(|c| c.is_ascii_hexdigit())
}

pub fn is_tac_address(q: &str) -> bool {
    q.starts_with("0x") && q.len() == 42 && q[2..].chars().all(|c| c.is_ascii_hexdigit())
}

pub fn is_ton_address(q: &str) -> bool {
    TonAddress::from_str(q).is_ok()
}

// This method converts TAC or TON addresses to a fixed format before storing them in the DB,
// (a consistent format is required for efficient searching)
// Returns Err() if the address is not in a recognized format
pub fn blockchain_address_to_db_format(addr: &str) -> anyhow::Result<String> {
    if is_tac_address(addr) {
        Ok(addr.to_lowercase())
    } else if let Ok(ton_addr) = TonAddress::from_str(addr) {
        Ok(ton_addr.to_base64_std_flags(false, false))
    } else {
        Err(anyhow!("unknown address format"))
    }
}