#[cfg(feature = "ethers-core")]
pub use ethers_core::types::Bytes;
#[cfg(feature = "ethers-core")]
pub use ethers_core::types::{deserialize_bytes, serialize_bytes};

#[cfg(not(feature = "ethers-core"))]
mod bytes;
#[cfg(not(feature = "ethers-core"))]
pub use crate::bytes::Bytes;
#[cfg(not(feature = "ethers-core"))]
pub use crate::bytes::{deserialize_bytes, serialize_bytes};

/// Allows to decode both "0x"-prefixed and non-prefixed hex strings
pub fn decode_hex(value: &str) -> Result<Vec<u8>, hex::FromHexError> {
    if let Some(value) = value.strip_prefix("0x") {
        hex::decode(value)
    } else {
        hex::decode(value)
    }
}

pub trait ToHex {
    /// Encodes given value as "0x"-prefixed hex string
    fn to_hex(&self) -> String;
}

impl<T: AsRef<[u8]>> ToHex for T {
    fn to_hex(&self) -> String {
        format!("0x{}", hex::encode(self))
    }
}
