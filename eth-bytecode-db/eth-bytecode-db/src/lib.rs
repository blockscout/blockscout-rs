pub mod search;
pub mod verification;

#[cfg(feature = "test-utils")]
pub mod tests;

mod metrics;

pub trait ToHex {
    fn to_hex(&self) -> String;
}

impl<T: AsRef<[u8]>> ToHex for T {
    fn to_hex(&self) -> String {
        format!("0x{}", hex::encode(self))
    }
}

pub trait FromHex {
    fn from_hex(value: &str) -> Result<Self, hex::FromHexError>
    where
        Self: Sized;
}

impl<T: From<Vec<u8>>> FromHex for T {
    fn from_hex(value: &str) -> Result<Self, hex::FromHexError>
    where
        Self: Sized,
    {
        if let Some(value) = value.strip_prefix("0x") {
            hex::decode(value)
        } else {
            hex::decode(value)
        }
        .map(|v| v.into())
    }
}

pub fn deserialize_bytes<'de, D>(d: D) -> Result<bytes::Bytes, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = <String as serde::Deserialize>::deserialize(d)?;
    if let Some(value) = value.strip_prefix("0x") {
        hex::decode(value)
    } else {
        hex::decode(&value)
    }
    .map(Into::into)
    .map_err(|e| serde::de::Error::custom(e.to_string()))
}
