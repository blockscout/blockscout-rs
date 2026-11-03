// SPDX-License-Identifier: LicenseRef-Blockscout

use anyhow::Result;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::NaiveDateTime;

use crate::utils::{bytes_to_naive_datetime, naive_datetime_to_bytes};

pub trait ListMarker: Sized {
    fn from_token(t: &str) -> anyhow::Result<Self>;
    fn token(&self) -> anyhow::Result<String>;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessagesPaginationLogic {
    pub timestamp: NaiveDateTime,
    pub message_id: u64,
    pub bridge_id: u32,
    pub direction: PaginationDirection,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransfersPaginationLogic {
    pub timestamp: NaiveDateTime,
    pub message_id: u64,
    pub bridge_id: u32,
    pub index: u64,
    pub direction: PaginationDirection,
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
}

/// Request sort for bridged-token stats (distinct from `stats.proto` enum wire 0..=3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BridgedTokensSortField {
    #[default]
    TotalTransfers = 0,
    OutputTransfers = 1,
    InputTransfers = 2,
    Name = 3,
}

impl BridgedTokensSortField {
    /// Maps `stats.proto` `BridgedTokensSort` wire values
    /// (TOTAL_TRANSFERS_COUNT=0, OUTPUT_TRANSFERS_COUNT=1, INPUT_TRANSFERS_COUNT=2, NAME=3).
    pub fn from_proto_sort(v: i32) -> Self {
        match v {
            0 => Self::TotalTransfers,
            1 => Self::OutputTransfers,
            2 => Self::InputTransfers,
            3 => Self::Name,
            _ => Self::TotalTransfers,
        }
    }
}

/// Request sort for `/stats/chains` (`stats.proto` `StatsChainsSort`; wire0 = unique transfer users).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum StatsChainsSortField {
    #[default]
    UniqueTransferUsersCount = 0,
}

impl StatsChainsSortField {
    /// Maps `stats.proto` `StatsChainsSort` wire values (UNIQUE_TRANSFER_USERS_COUNT=0).
    pub fn from_proto_sort(v: i32) -> Self {
        match v {
            0 => Self::UniqueTransferUsersCount,
            _ => Self::UniqueTransferUsersCount,
        }
    }
}

/// Shared request order for statistics list endpoints (`stats.proto` `SortOrder`: 0=DESC, 1=ASC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum StatsSortOrder {
    #[default]
    Desc = 1,
    Asc = 2,
}

impl StatsSortOrder {
    /// Maps `stats.proto` `SortOrder` (DESC=0, ASC=1).
    pub fn from_proto_order(v: i32) -> anyhow::Result<Self> {
        match v {
            0 => Ok(Self::Desc),
            1 => Ok(Self::Asc),
            _ => Err(anyhow::anyhow!("Invalid value for order")),
        }
    }
}

/// Keyset cursor for `/stats/bridged-tokens` (packed into `page_token`).
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

impl ListMarker for BridgedTokensPaginationLogic {
    fn from_token(token: &str) -> anyhow::Result<Self> {
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

    fn token(&self) -> anyhow::Result<String> {
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
}

const SC_PAGE_TOKEN_VERSION: u8 = 1;

/// Keyset cursor for `/stats/chains` (packed into `page_token`).
#[derive(Debug, Clone, PartialEq)]
pub struct StatsChainsPaginationLogic {
    pub direction: PaginationDirection,
    pub count: i64,
    pub chain_id: i64,
}

impl ListMarker for StatsChainsPaginationLogic {
    fn from_token(token: &str) -> anyhow::Result<Self> {
        let decoded = URL_SAFE_NO_PAD
            .decode(token)
            .map_err(|e| anyhow::anyhow!("Invalid base64 token: {e}"))?;
        let d = decoded.as_slice();
        if d.len() != 18 || d[0] != SC_PAGE_TOKEN_VERSION {
            return Err(anyhow::anyhow!("Invalid stats-chains page token"));
        }
        let direction = PaginationDirection::from_u8(d[1])?;
        let count = i64::from_be_bytes(d[2..10].try_into().unwrap());
        let chain_id = i64::from_be_bytes(d[10..18].try_into().unwrap());
        Ok(Self {
            direction,
            count,
            chain_id,
        })
    }

    fn token(&self) -> anyhow::Result<String> {
        let mut out = [0u8; 18];
        out[0] = SC_PAGE_TOKEN_VERSION;
        out[1] = self.direction.to_u8();
        out[2..10].copy_from_slice(&self.count.to_be_bytes());
        out[10..18].copy_from_slice(&self.chain_id.to_be_bytes());
        Ok(URL_SAFE_NO_PAD.encode(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_chains_sort_wire_values_match_proto() {
        assert_eq!(
            StatsChainsSortField::from_proto_sort(0),
            StatsChainsSortField::UniqueTransferUsersCount
        );
        assert_eq!(
            StatsChainsSortField::from_proto_sort(99),
            StatsChainsSortField::UniqueTransferUsersCount
        );
    }

    #[test]
    fn bridged_tokens_sort_wire_values_match_proto() {
        assert_eq!(
            BridgedTokensSortField::from_proto_sort(0),
            BridgedTokensSortField::TotalTransfers
        );
        assert_eq!(
            BridgedTokensSortField::from_proto_sort(1),
            BridgedTokensSortField::OutputTransfers
        );
        assert_eq!(
            BridgedTokensSortField::from_proto_sort(2),
            BridgedTokensSortField::InputTransfers
        );
        assert_eq!(
            BridgedTokensSortField::from_proto_sort(3),
            BridgedTokensSortField::Name
        );
        assert_eq!(
            BridgedTokensSortField::from_proto_sort(99),
            BridgedTokensSortField::TotalTransfers
        );
    }

    /// 2026-01-02T00:30:00Z.
    fn golden_timestamp() -> NaiveDateTime {
        chrono::DateTime::from_timestamp(1_767_313_800, 0)
            .expect("valid timestamp")
            .naive_utc()
    }

    // Golden vectors pin the token byte formats: tokens already handed to
    // clients must keep decoding after any refactor. Never edit a literal to
    // match a changed encoder.

    #[test]
    fn messages_page_token_golden_vector() {
        const TOKEN: &str = "GIbCiZelUAAAAAAAAAASNAAAAAEA";
        let marker = MessagesPaginationLogic {
            timestamp: golden_timestamp(),
            message_id: 0x1234,
            bridge_id: 1,
            direction: PaginationDirection::Next,
        };
        assert_eq!(marker.token().unwrap(), TOKEN);
        assert_eq!(MessagesPaginationLogic::from_token(TOKEN).unwrap(), marker);
    }

    #[test]
    fn transfers_page_token_golden_vector() {
        const TOKEN: &str = "GIbCiZelUAAAAAAAAAASNAAAAAEAAAAAAAAABwA";
        let marker = TransfersPaginationLogic {
            timestamp: golden_timestamp(),
            message_id: 0x1234,
            bridge_id: 1,
            index: 7,
            direction: PaginationDirection::Next,
        };
        assert_eq!(marker.token().unwrap(), TOKEN);
        assert_eq!(TransfersPaginationLogic::from_token(TOKEN).unwrap(), marker);
    }

    #[test]
    fn stats_chains_page_token_golden_vector() {
        const TOKEN: &str = "AQAAAAAAAAAAMgAAAAAAAABk";
        let marker = StatsChainsPaginationLogic {
            direction: PaginationDirection::Next,
            count: 50,
            chain_id: 100,
        };
        assert_eq!(marker.token().unwrap(), TOKEN);
        assert_eq!(
            StatsChainsPaginationLogic::from_token(TOKEN).unwrap(),
            marker
        );
    }

    #[test]
    fn bridged_tokens_page_token_golden_vector() {
        const TOKEN: &str = "AQAAAAAAAAAAKgAABHVzZGMAAAAAAAAABw";
        let marker = BridgedTokensPaginationLogic {
            direction: PaginationDirection::Next,
            stats_asset_id: 42,
            name_blank: false,
            name_sort: "usdc".to_string(),
            count: 7,
        };
        assert_eq!(marker.token().unwrap(), TOKEN);
        assert_eq!(
            BridgedTokensPaginationLogic::from_token(TOKEN).unwrap(),
            marker
        );
    }

    #[test]
    fn bridged_tokens_blank_name_page_token_golden_vector() {
        const TOKEN: &str = "AQEAAAAAAAAAKgEAAAAAAAAAAAAA";
        let marker = BridgedTokensPaginationLogic {
            direction: PaginationDirection::Prev,
            stats_asset_id: 42,
            name_blank: true,
            name_sort: String::new(),
            count: 0,
        };
        assert_eq!(marker.token().unwrap(), TOKEN);
        assert_eq!(
            BridgedTokensPaginationLogic::from_token(TOKEN).unwrap(),
            marker
        );
    }
}
