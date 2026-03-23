use std::fmt;

use anyhow::Result;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::NaiveDateTime;
use interchain_indexer_proto::blockscout::interchain_indexer::v1::{
    BridgedTokensListPagination, Pagination,
};

use crate::utils::{
    bytes_to_naive_datetime, naive_datetime_to_bytes, naive_datetime_to_nanos,
    nanos_to_naive_datetime, to_hex_prefixed, u64_from_hex_prefixed,
};

pub trait ListMarker: Sized {
    fn from_token(t: &str) -> anyhow::Result<Self>;
    fn token(&self) -> anyhow::Result<String>;

    fn to_proto(&self, use_pagination_token: bool) -> Pagination;

    //fn from_proto(p: ) -> anyhow::Result<Self>;
}

pub struct OutputPagination<P> {
    pub prev_marker: Option<P>,
    pub next_marker: Option<P>,
}

impl<P> Default for OutputPagination<P> {
    fn default() -> Self {
        Self {
            prev_marker: None,
            next_marker: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    pub fn to_u8(self) -> u8 {
        match self {
            PaginationDirection::Next => 0,
            PaginationDirection::Prev => 1,
        }
    }
}

impl fmt::Display for PaginationDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PaginationDirection::Next => "next",
            PaginationDirection::Prev => "prev",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MessagesPaginationLogic {
    pub timestamp: NaiveDateTime,
    pub message_id: u64,
    pub bridge_id: u32,
    pub direction: PaginationDirection,
}

impl MessagesPaginationLogic {
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
        naive_datetime_to_nanos(self.timestamp)
    }

    pub fn get_message_id(&self) -> String {
        to_hex_prefixed(&self.message_id.to_be_bytes())
    }
}

impl ListMarker for MessagesPaginationLogic {
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

    // Create output pagination proto struct
    fn to_proto(&self, use_pagination_token: bool) -> Pagination {
        if use_pagination_token {
            Pagination {
                page_token: Some(self.token().unwrap()),
                ..Default::default()
            }
        } else {
            Pagination {
                timestamp: Some(self.get_timestamp_ns().unwrap() as u64),
                message_id: Some(self.get_message_id()),
                bridge_id: Some(self.bridge_id),
                direction: Some(self.direction.to_string()),
                ..Default::default()
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TransfersPaginationLogic {
    pub timestamp: NaiveDateTime,
    pub message_id: u64,
    pub bridge_id: u32,
    pub index: u64,
    pub direction: PaginationDirection,
}

impl TransfersPaginationLogic {
    pub fn new(
        timestamp_ns: i64,
        message_id: String,
        bridge_id: u32,
        transfer_id: u64,
        direction: PaginationDirection,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            timestamp: nanos_to_naive_datetime(timestamp_ns)?,
            message_id: u64_from_hex_prefixed(&message_id)?,
            bridge_id,
            index: transfer_id,
            direction,
        })
    }

    pub fn get_timestamp_ns(&self) -> anyhow::Result<i64> {
        naive_datetime_to_nanos(self.timestamp)
    }

    pub fn get_message_id(&self) -> String {
        to_hex_prefixed(&self.message_id.to_be_bytes())
    }
}

impl ListMarker for TransfersPaginationLogic {
    fn from_token(t: &str) -> anyhow::Result<Self> {
        let decoded = URL_SAFE_NO_PAD
            .decode(t)
            .map_err(|e| anyhow::anyhow!("Invalid base64 token: {e}"))?;

        if decoded.len() != 29 {
            return Err(anyhow::anyhow!(
                "Invalid token length: expected 29, got {}",
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

        let transfer_id_bytes: [u8; 8] = decoded[20..28]
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid message_id bytes"))?;
        let index = u64::from_be_bytes(transfer_id_bytes);

        let direction = PaginationDirection::from_u8(decoded[28])?;

        Ok(Self {
            timestamp,
            message_id,
            bridge_id,
            index,
            direction,
        })
    }

    // serialize into the string URL-friedly token [base64 string]
    fn token(&self) -> anyhow::Result<String> {
        let mut buf = [0u8; 29];

        buf[0..8].copy_from_slice(&naive_datetime_to_bytes(self.timestamp)?);
        buf[8..16].copy_from_slice(&self.message_id.to_be_bytes());
        buf[16..20].copy_from_slice(&self.bridge_id.to_be_bytes());
        buf[20..28].copy_from_slice(&self.index.to_be_bytes());
        buf[28] = self.direction.to_u8();

        Ok(URL_SAFE_NO_PAD.encode(buf))
    }

    // Create output pagination proto struct
    fn to_proto(&self, use_pagination_token: bool) -> Pagination {
        if use_pagination_token {
            Pagination {
                page_token: Some(self.token().unwrap()),
                ..Default::default()
            }
        } else {
            Pagination {
                timestamp: Some(self.get_timestamp_ns().unwrap() as u64),
                message_id: Some(self.get_message_id()),
                bridge_id: Some(self.bridge_id),
                index: Some(self.index),
                direction: Some(self.direction.to_string()),
                ..Default::default()
            }
        }
    }
}

/// Request sort for bridged-token stats (distinct from `stats.proto` enum wire 0..=3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BridgedTokensSortField {
    #[default]
    Name = 1,
    InputTransfers = 2,
    OutputTransfers = 3,
    TotalTransfers = 4,
}

impl BridgedTokensSortField {
    /// Maps `stats.proto` `BridgedTokensSort` wire values (NAME=0 … TOTAL_TRANSFERS_COUNT=3).
    pub fn from_proto_sort(v: i32) -> Self {
        match v {
            0 => Self::Name,
            1 => Self::InputTransfers,
            2 => Self::OutputTransfers,
            3 => Self::TotalTransfers,
            _ => Self::Name,
        }
    }
}

/// Request order for bridged-token stats (`stats.proto`: 0=DESC, 1=ASC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BridgedTokensSortOrder {
    #[default]
    Desc = 1,
    Asc = 2,
}

impl BridgedTokensSortOrder {
    /// Maps `stats.proto` `BridgedTokensOrder` (DESC=0, ASC=1).
    pub fn from_proto_order(v: i32) -> anyhow::Result<Self> {
        match v {
            0 => Ok(Self::Desc),
            1 => Ok(Self::Asc),
            _ => Err(anyhow::anyhow!("Invalid value for order")),
        }
    }
}

/// Keyset cursor for `/stats/bridged-tokens` (packed into `page_token` or raw `BridgedTokensListPagination`).
/// Does not embed chain, sort, or order — callers must keep request parameters aligned with the query.
#[derive(Debug, Clone, PartialEq)]
pub struct BridgedTokensPaginationLogic {
    pub direction: PaginationDirection,
    pub stats_asset_id: i64,
    /// `true` when `stats_assets.name` is NULL or empty/whitespace — sorts after all non-blank names.
    pub name_blank: bool,
    /// Sort key for name (ignored when `name_blank`); empty string when blank.
    pub name_sort: String,
    /// Value of the sorted count column when sorting by input/output/total; otherwise `0`.
    pub count: i64,
}

const BT_PAGE_TOKEN_VERSION: u8 = 1;
const BT_MAX_NAME_CURSOR_BYTES: usize = 512;

impl BridgedTokensPaginationLogic {
    pub fn from_token(token: &str) -> anyhow::Result<Self> {
        let decoded = URL_SAFE_NO_PAD
            .decode(token)
            .map_err(|e| anyhow::anyhow!("Invalid base64 token: {e}"))?;
        let d = decoded.as_slice();
        if d.is_empty() || d[0] != BT_PAGE_TOKEN_VERSION {
            return Err(anyhow::anyhow!("Invalid bridged-tokens page token version"));
        }
        let mut i = 1usize;
        if d.len() < i + 1 {
            return Err(anyhow::anyhow!(
                "Invalid bridged-tokens page token (truncated)"
            ));
        }
        let direction = PaginationDirection::from_u8(d[i])?;
        i += 1;
        if d.len() < i + 8 {
            return Err(anyhow::anyhow!(
                "Invalid bridged-tokens page token (truncated)"
            ));
        }
        let stats_asset_id = i64::from_be_bytes(d[i..i + 8].try_into().unwrap());
        i += 8;
        if d.len() < i + 1 {
            return Err(anyhow::anyhow!(
                "Invalid bridged-tokens page token (truncated)"
            ));
        }
        let name_blank = d[i] != 0;
        i += 1;
        if d.len() < i + 2 {
            return Err(anyhow::anyhow!(
                "Invalid bridged-tokens page token (truncated)"
            ));
        }
        let name_len = u16::from_be_bytes(d[i..i + 2].try_into().unwrap()) as usize;
        i += 2;
        if name_len > BT_MAX_NAME_CURSOR_BYTES {
            return Err(anyhow::anyhow!(
                "Invalid bridged-tokens page token (name too long)"
            ));
        }
        if d.len() < i + name_len {
            return Err(anyhow::anyhow!(
                "Invalid bridged-tokens page token (truncated)"
            ));
        }
        let name_sort = std::str::from_utf8(&d[i..i + name_len])
            .map_err(|_| anyhow::anyhow!("Invalid bridged-tokens page token (name utf-8)"))?
            .to_string();
        i += name_len;
        if d.len() < i + 8 {
            return Err(anyhow::anyhow!(
                "Invalid bridged-tokens page token (truncated)"
            ));
        }
        let count = i64::from_be_bytes(d[i..i + 8].try_into().unwrap());
        if i + 8 != d.len() {
            return Err(anyhow::anyhow!(
                "Invalid bridged-tokens page token (trailing data)"
            ));
        }
        Ok(Self {
            direction,
            stats_asset_id,
            name_blank,
            name_sort,
            count,
        })
    }

    pub fn token(&self) -> anyhow::Result<String> {
        let name_bytes = self.name_sort.as_bytes();
        if name_bytes.len() > BT_MAX_NAME_CURSOR_BYTES {
            return Err(anyhow::anyhow!("name cursor exceeds max length"));
        }
        let mut out = Vec::with_capacity(1 + 1 + 8 + 1 + 2 + name_bytes.len() + 8);
        out.push(BT_PAGE_TOKEN_VERSION);
        out.push(self.direction.to_u8());
        out.extend_from_slice(&self.stats_asset_id.to_be_bytes());
        out.push(u8::from(self.name_blank));
        out.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(&self.count.to_be_bytes());
        Ok(URL_SAFE_NO_PAD.encode(out))
    }

    /// Token mode: only `page_token` is set. Raw mode: `direction` + bridged-tokens cursor fields (see `stats.proto`).
    pub fn to_list_pagination_proto(
        &self,
        use_pagination_token: bool,
    ) -> BridgedTokensListPagination {
        if use_pagination_token {
            BridgedTokensListPagination {
                page_token: Some(self.token().unwrap_or_default()),
                ..Default::default()
            }
        } else {
            BridgedTokensListPagination {
                direction: Some(self.direction.to_string()),
                asset_id: Some(self.stats_asset_id),
                name_blank: Some(self.name_blank),
                name: if self.name_blank {
                    None
                } else {
                    Some(self.name_sort.clone())
                },
                count: Some(self.count as u64),
                ..Default::default()
            }
        }
    }

    /// Raw continuation: same keys as [`BridgedTokensListPagination`] (request uses the same flattened names as that message).
    pub fn try_from_list_pagination_proto(
        lp: &BridgedTokensListPagination,
    ) -> anyhow::Result<Option<Self>> {
        let has_cursor = lp.asset_id.is_some()
            || lp.name_blank.is_some()
            || lp.count.is_some()
            || lp.name.as_ref().is_some_and(|s| !s.is_empty());
        if lp.direction.is_none() && !has_cursor {
            return Ok(None);
        }
        let dir_str = lp.direction.as_deref().ok_or_else(|| {
            anyhow::anyhow!("direction is required when continuing a page (raw mode)")
        })?;
        let direction = PaginationDirection::from_string(dir_str)?;
        let stats_asset_id = lp
            .asset_id
            .ok_or_else(|| anyhow::anyhow!("asset_id is required when paginating (raw mode)"))?;
        let name_blank = lp
            .name_blank
            .ok_or_else(|| anyhow::anyhow!("name_blank is required when paginating (raw mode)"))?;
        let count = lp.count.map(|c| c as i64).unwrap_or(0);
        let name_sort = if name_blank {
            String::new()
        } else {
            lp.name.clone().unwrap_or_default()
        };
        Ok(Some(Self {
            direction,
            stats_asset_id,
            name_blank,
            name_sort,
            count,
        }))
    }
}
