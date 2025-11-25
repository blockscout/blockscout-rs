use anyhow::Result;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::NaiveDateTime;

use crate::utils::{
    bytes_to_naive_datetime, naive_datetime_to_bytes, naive_datetime_to_nanos,
    nanos_to_naive_datetime, to_hex_prefixed, u64_from_hex_prefixed,
};

pub trait ListMarker: Sized {
    fn from_token(t: &str) -> anyhow::Result<Self>;
    fn token(&self) -> anyhow::Result<String>;
}

pub struct OutputPagination<P: ListMarker> {
    pub prev_marker: Option<P>,
    pub next_marker: Option<P>,
}

impl<P: ListMarker> Default for OutputPagination<P> {
    fn default() -> Self {
        Self {
            prev_marker: None,
            next_marker: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PaginationDirection {
    Next,
    Prev,
}

impl PaginationDirection {
    pub fn from_string(s: &str) -> Result<Self> {
        match s {
            "next" => Ok(PaginationDirection::Next),
            "prev" => Ok(PaginationDirection::Prev),
            _ => Err(anyhow::anyhow!("Invalid value for direction")),
        }
    }

    pub fn from_u8(v: u8) -> Result<Self> {
        match v {
            0 => Ok(PaginationDirection::Next),
            1 => Ok(PaginationDirection::Prev),
            _ => Err(anyhow::anyhow!("Invalid value for direction")),
        }
    }

    pub fn to_string(self) -> String {
        match self {
            PaginationDirection::Next => "next".to_string(),
            PaginationDirection::Prev => "prev".to_string(),
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            PaginationDirection::Next => 0,
            PaginationDirection::Prev => 1,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MessagePaginationLogic {
    pub timestamp: NaiveDateTime,
    pub message_id: u64,
    pub bridge_id: u32,
    pub direction: PaginationDirection,
}

impl MessagePaginationLogic {
    pub fn new(
        timestamp_ns: i64,
        message_id: String,
        bridge_id: u32,
        direction: PaginationDirection,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            timestamp: nanos_to_naive_datetime(timestamp_ns)?,
            message_id: u64_from_hex_prefixed(&message_id)?,
            bridge_id,
            direction,
        })
    }

    pub fn get_timestamp_ns(&self) -> anyhow::Result<i64> {
        Ok(naive_datetime_to_nanos(self.timestamp)?)
    }

    pub fn get_message_id(&self) -> String {
        to_hex_prefixed(&self.message_id.to_be_bytes())
    }
}

impl ListMarker for MessagePaginationLogic {
    fn from_token(t: &str) -> anyhow::Result<Self> {
        let decoded = URL_SAFE_NO_PAD
            .decode(t)
            .map_err(|e| anyhow::anyhow!("Invalid base64 token: {e}"))?;

        if decoded.len() != 21 {
            return Err(anyhow::anyhow!(
                "Invalid token length: expected 21, got {}",
                decoded.len()
            ));
        }

        let timestamp_bytes: [u8; 8] = decoded[0..8]
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid timestamp bytes"))?;
        let timestamp = bytes_to_naive_datetime(timestamp_bytes)?;

        let message_id_bytes: [u8; 8] = decoded[8..16]
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid message_id bytes"))?;
        let message_id = u64::from_be_bytes(message_id_bytes);

        let bridge_id_bytes: [u8; 4] = decoded[16..20]
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid bridge_id bytes"))?;
        let bridge_id = u32::from_be_bytes(bridge_id_bytes);

        let direction = PaginationDirection::from_u8(decoded[20])?;

        Ok(Self {
            timestamp,
            message_id,
            bridge_id,
            direction,
        })
    }

    // serialize into the string URL-friedly token [base64 string]
    fn token(&self) -> anyhow::Result<String> {
        let mut buf = [0u8; 21];

        buf[0..8].copy_from_slice(&naive_datetime_to_bytes(self.timestamp)?);
        buf[8..16].copy_from_slice(&self.message_id.to_be_bytes());
        buf[16..20].copy_from_slice(&self.bridge_id.to_be_bytes());
        buf[20] = self.direction.to_u8();

        Ok(URL_SAFE_NO_PAD.encode(buf))
    }
}
