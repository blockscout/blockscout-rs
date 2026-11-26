// SPDX-License-Identifier: LicenseRef-Blockscout

use sea_orm::ActiveValue::Set;

use crate::{
    bridge_contracts, crosschain_transfers, indexer_checkpoints,
    sea_orm_active_enums::{MessageStatus, TransferAssetLinkage},
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
/// Existing explicit registry types take precedence over this
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

/// Terminality of `crosschain_messages.status` (ADR-021). This is the only
/// definition of it: the upsert guard (`crosschain_messages_on_conflict`) and
/// buffer eviction (`classify_item`) are built from it.
///
/// The rule:
/// 1. The base set means the same on every bridge: `initiated` (the default)
///    is non-terminal; `completed` and `failed` are terminal.
/// 2. Terminal means the row never again holds a non-terminal status. A move
///    between `completed` and `failed` happens only for a protocol-defined
///    retry (Avalanche `failed -> completed`). Terminal is relative to the
///    indexer's confirmation boundary: a reorg below it is outside this
///    model.
/// 3. Every value a protocol adds is non-terminal: it refines the
///    not-yet-terminal state. A terminal outcome is stored as `completed` or
///    `failed`, with the protocol's reason in `protocol_metadata` (`extra`).
/// 4. `ConsolidatedMessage::is_final` implies a terminal status, not the
///    converse.
/// 5. Status describes the protocol lifecycle only (ADR-004).
/// 6. A value is added in the change that first writes it: PostgreSQL
///    cannot drop an enum value. Checklist: `.memory-bank/rules/database.md`,
///    "Message Status Values".
///
/// A new variant does not compile until it has an arm here. Its arm is
/// `false` (clause 3); the test below rejects anything else.
impl MessageStatus {
    pub fn is_terminal(&self) -> bool {
        match self {
            Self::Completed | Self::Failed => true,
            Self::Initiated | Self::ReadyToClaim | Self::PendingFinality | Self::Invalidated => {
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::Iterable;

    use crate::sea_orm_active_enums::MessageStatus;

    #[test]
    fn test_message_status_is_terminal_only_for_completed_and_failed() {
        let terminal: Vec<MessageStatus> = MessageStatus::iter()
            .filter(MessageStatus::is_terminal)
            .collect();
        assert_eq!(
            terminal,
            vec![MessageStatus::Completed, MessageStatus::Failed],
            "ADR-021: every value a protocol adds is non-terminal; store a terminal \
             outcome as `completed` or `failed` with the reason in `protocol_metadata`"
        );
    }
}
