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
