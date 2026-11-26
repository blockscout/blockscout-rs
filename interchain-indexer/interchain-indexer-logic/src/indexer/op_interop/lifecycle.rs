// SPDX-License-Identifier: LicenseRef-Blockscout

//! The status fold of an OP interop message (ADR-022): a pure function of the
//! `SentMessage` and `RelayedMessage` observations, buffer legs or ledger rows
//! alike, so the stored status can be checked against the ledger.

use std::cmp::Reverse;

use alloy::primitives::B256;
use interchain_indexer_entity::sea_orm_active_enums::MessageStatus;

use super::types::{DestinationLeg, SourceLeg};
use crate::protocol_metadata::OpInteropEventState;

/// One `SentMessage` or `RelayedMessage` observation: a buffer leg or a ledger
/// row. Executing rows are not folded: they share their relay's block, so
/// their state is the relay's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LegView {
    pub(super) state: OpInteropEventState,
    pub(super) block_number: u64,
    pub(super) block_hash: B256,
    pub(super) log_index: u64,
}

impl LegView {
    pub(super) fn of_source(leg: &SourceLeg) -> Self {
        Self {
            state: leg.state,
            block_number: leg.log.block_number,
            block_hash: leg.log.block_hash,
            log_index: leg.log.log_index,
        }
    }

    pub(super) fn of_destination(leg: &DestinationLeg) -> Self {
        Self {
            state: leg.state,
            block_number: leg.log.block_number,
            block_hash: leg.log.block_hash,
            log_index: leg.log.log_index,
        }
    }

    /// Test oracle: the view of a ledger row; `None` for an unknown state or a
    /// value that does not fit.
    #[cfg(test)]
    pub(super) fn of_ledger(
        row: &interchain_indexer_entity::op_interop_events::Model,
    ) -> Option<Self> {
        Some(Self {
            state: OpInteropEventState::parse(&row.state)?,
            block_number: u64::try_from(row.block_number).ok()?,
            block_hash: B256::try_from(row.block_hash.as_slice()).ok()?,
            log_index: u64::try_from(row.log_index).ok()?,
        })
    }

    /// Confirmed before provisional, then the highest block, then the lowest log
    /// index, then the lowest block hash.
    fn order_key(&self) -> (bool, Reverse<u64>, u64, B256) {
        (
            self.state != OpInteropEventState::Confirmed,
            Reverse(self.block_number),
            self.log_index,
            self.block_hash,
        )
    }
}

fn best(legs: &[LegView], keep: impl Fn(&LegView) -> bool) -> Option<usize> {
    legs.iter()
        .enumerate()
        .filter(|(_, leg)| keep(leg))
        .min_by_key(|(_, leg)| leg.order_key())
        .map(|(index, _)| index)
}

/// The candidate a side stands for: confirmed before provisional, then the
/// highest block, then the lowest log index, then the lowest block hash.
/// Never a retracted one. Content order, not arrival order, so the same fold
/// runs over ledger rows.
pub(super) fn active(legs: &[LegView]) -> Option<usize> {
    best(legs, |leg| leg.state != OpInteropEventState::Retracted)
}

/// What the row and `extra` show: the active candidate, else the retracted one
/// with the highest block (same tie-breaks).
pub(super) fn shown(legs: &[LegView]) -> Option<usize> {
    active(legs).or_else(|| best(legs, |_| true))
}

/// The message status the candidates imply; `None` means no canonical row.
pub(super) fn status(sources: &[LegView], relays: &[LegView]) -> Option<MessageStatus> {
    if sources.is_empty() {
        return None;
    }
    let Some(source) = active(sources).map(|index| sources[index]) else {
        return Some(MessageStatus::Invalidated);
    };
    let Some(relay) = active(relays).map(|index| relays[index]) else {
        return Some(MessageStatus::Initiated);
    };
    Some(
        if source.state == OpInteropEventState::Confirmed
            && relay.state == OpInteropEventState::Confirmed
        {
            MessageStatus::Completed
        } else {
            MessageStatus::PendingFinality
        },
    )
}

/// Test oracle: `status` over the `sent_message` / `relayed_message` rows of
/// one message.
#[cfg(test)]
pub(super) fn status_of_ledger(
    rows: &[interchain_indexer_entity::op_interop_events::Model],
) -> Option<MessageStatus> {
    let views = |kind: &str| -> Vec<LegView> {
        rows.iter()
            .filter(|row| row.event_kind == kind)
            .filter_map(LegView::of_ledger)
            .collect()
    };
    status(&views("sent_message"), &views("relayed_message"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(state: OpInteropEventState, block_number: u64, log_index: u64, hash: u8) -> LegView {
        LegView {
            state,
            block_number,
            block_hash: B256::repeat_byte(hash),
            log_index,
        }
    }

    const P: OpInteropEventState = OpInteropEventState::Provisional;
    const C: OpInteropEventState = OpInteropEventState::Confirmed;
    const R: OpInteropEventState = OpInteropEventState::Retracted;

    #[test]
    fn test_status_table() {
        let s = |state| vec![view(state, 10, 1, 1)];
        // No source: no row, whatever the relays say.
        assert_eq!(status(&[], &[]), None);
        assert_eq!(status(&[], &s(C)), None);
        // Every source retracted.
        assert_eq!(status(&s(R), &[]), Some(MessageStatus::Invalidated));
        assert_eq!(status(&s(R), &s(C)), Some(MessageStatus::Invalidated));
        // An active source, no active relay.
        assert_eq!(status(&s(P), &[]), Some(MessageStatus::Initiated));
        assert_eq!(status(&s(C), &s(R)), Some(MessageStatus::Initiated));
        // Both confirmed.
        assert_eq!(status(&s(C), &s(C)), Some(MessageStatus::Completed));
        // Relayed, an active leg not confirmed.
        assert_eq!(status(&s(P), &s(C)), Some(MessageStatus::PendingFinality));
        assert_eq!(status(&s(C), &s(P)), Some(MessageStatus::PendingFinality));
        assert_eq!(status(&s(P), &s(P)), Some(MessageStatus::PendingFinality));
        // A confirmed relay beats a provisional one at the same side.
        assert_eq!(
            status(&s(C), &[view(P, 20, 3, 2), view(C, 10, 3, 3)]),
            Some(MessageStatus::Completed)
        );
        // A retracted source beside a live one.
        assert_eq!(
            status(&[view(R, 5, 1, 1), view(C, 9, 1, 2)], &[]),
            Some(MessageStatus::Initiated)
        );
    }

    #[test]
    fn test_active_and_shown_order() {
        // Confirmed beats provisional at a lower block.
        let legs = [view(P, 20, 1, 1), view(C, 10, 1, 2)];
        assert_eq!(active(&legs), Some(1));
        // Among provisional, the highest block wins.
        let legs = [view(P, 10, 1, 1), view(P, 20, 1, 2)];
        assert_eq!(active(&legs), Some(1));
        // At one height: the lowest log index, then the lowest block hash.
        let legs = [view(P, 10, 5, 1), view(P, 10, 2, 9)];
        assert_eq!(active(&legs), Some(1));
        let legs = [view(P, 10, 2, 9), view(P, 10, 2, 3)];
        assert_eq!(active(&legs), Some(1));
        // `shown` falls back to the highest retracted; `active` never returns it.
        let legs = [view(R, 10, 1, 1), view(R, 30, 1, 2), view(R, 20, 1, 3)];
        assert_eq!(active(&legs), None);
        assert_eq!(shown(&legs), Some(1));
        // `shown` prefers an active candidate over a higher retracted one.
        let legs = [view(R, 30, 1, 1), view(P, 10, 1, 2)];
        assert_eq!(shown(&legs), Some(1));
        assert_eq!(active(&[]), None);
        assert_eq!(shown(&[]), None);
    }

    #[test]
    fn test_order_is_content_not_arrival() {
        // The winner is first, so "the last non-retracted" would fail.
        assert_eq!(active(&[view(C, 10, 1, 1), view(P, 20, 1, 2)]), Some(0));
        assert_eq!(active(&[view(P, 20, 1, 1), view(P, 10, 1, 2)]), Some(0));
        // One block and hash: the lower log index.
        assert_eq!(active(&[view(P, 10, 2, 1), view(P, 10, 9, 1)]), Some(0));
        // One height and log index: the lower block hash.
        assert_eq!(active(&[view(P, 10, 2, 3), view(P, 10, 2, 9)]), Some(0));
        // The fold over a reversed-arrival relay list.
        assert_eq!(
            status(&[view(C, 5, 1, 1)], &[view(C, 10, 3, 2), view(P, 20, 3, 3)]),
            Some(MessageStatus::Completed)
        );
    }
}
