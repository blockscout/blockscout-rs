// SPDX-License-Identifier: LicenseRef-Blockscout

use sea_orm::ActiveValue::Set;

use crate::{
    bridge_contracts, crosschain_transfers, indexer_checkpoints,
    sea_orm_active_enums::TransferAssetLinkage,
};

/// Every transfer an indexer builds must state its asset linkage. There is no
/// default: `NULL` means "the indexer does not know yet" and defers the row
/// from stats projection indefinitely, so an accidentally-omitted value is a
/// silent stats outage for that bridge.
///
/// Start every `crosschain_transfers::ActiveModel` from here rather than from
/// `Default::default()`. This is the correct default path, not enforcement —
/// see the chokepoint check in `message_buffer::persistence`.
pub fn new_transfer(linkage: TransferAssetLinkage) -> crosschain_transfers::ActiveModel {
    crosschain_transfers::ActiveModel {
        asset_linkage: Set(Some(linkage)),
        stats_processed: Set(0),
        src_stats_asset_id: Set(None),
        dst_stats_asset_id: Set(None),
        ..Default::default()
    }
}

impl indexer_checkpoints::Model {
    pub fn validated_realtime_cursor(&self) -> u64 {
        self.realtime_cursor.max(0) as u64
    }

    pub fn validated_catchup_cursor(&self) -> u64 {
        self.catchup_max_cursor.max(0) as u64
    }

    pub fn validated_catchup_min_cursor(&self) -> u64 {
        self.catchup_min_cursor.max(0) as u64
    }
}

impl bridge_contracts::Model {
    pub fn validated_started_at_block(&self) -> u64 {
        self.started_at_block.unwrap_or(0).max(0) as u64
    }
}

/// Classify a storage key before optional contract metadata is available.
/// The exact twenty-byte zero address is reserved for a chain's native coin.
/// Existing explicit registry types (including NFTs) take precedence over this
/// fallback; all currently indexed contract tokens are ERC-20.
impl crate::sea_orm_active_enums::TokenType {
    pub fn from_address(address: &[u8]) -> Self {
        if address == [0; 20] {
            Self::Native
        } else {
            Self::Erc20
        }
    }
}
