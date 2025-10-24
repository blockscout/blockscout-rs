use alloy_primitives::Address;
use base32ct::{Base32Unpadded, Encoding};
use std::str::FromStr;

pub fn try_filecoin_address_to_evm_address(address: &str) -> Option<Address> {
    // ID addresses
    if let Some(actor_id) = address.strip_prefix("f0") {
        let actor_id = actor_id.parse::<u64>().ok()?;
        let address =
            Address::from_str(&format!("0xff0000000000000000000000{:016X}", actor_id)).ok()?;

        return Some(address);
    };

    // Ethereum-compatible addresses
    if let Some(payload) = address.strip_prefix("f410f") {
        let decoded = Base32Unpadded::decode_vec(payload).ok()?;

        // address (20 bytes) + checksum (4 bytes)
        if decoded.len() != 24 {
            return None;
        }

        let address = Address::try_from(&decoded[..20]).ok()?;

        return Some(address);
    };

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_filecoin_address_to_evm_address() {
        assert_eq!(
            try_filecoin_address_to_evm_address("f410falck3ysg7e2k4outtq2r24ytd66cuddydnoga6a")
                .unwrap(),
            Address::from_str("0x02c4ade246f934aE3a939c351d73131FBc2A0c78").unwrap()
        );

        assert_eq!(
            try_filecoin_address_to_evm_address("f0120").unwrap(),
            Address::from_str("0xFF00000000000000000000000000000000000078").unwrap()
        );
    }
}
