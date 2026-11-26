// SPDX-License-Identifier: LicenseRef-Blockscout

//! The confirmation sweep (ADR-022): retracts the provisional ledger rows the
//! confirmed stream passed without delivering. A provisional row at or below
//! the frontier, outside every open failed range, sits at a height the
//! confirmed stream finished; had its log been in the canonical block, its row
//! would be `confirmed`.

use std::collections::HashMap;

use alloy::primitives::B256;
use anyhow::{Context, Result};

use crate::message_buffer::{
    Key, ProvisionalEvent, provisional_op_interop_events, retract_op_interop_events,
};

use super::{indexer::RunContext, metrics, types::Message, version::OpInteropEvent};

/// At most this many rows per sweep; the rest waits for the next call. Bounds
/// the time `alter_existing_durably` holds the maintenance lock.
pub(super) const SWEEP_BATCH: i64 = 1000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SweepOutcome {
    /// Selected provisional rows, every one `retracted` when the sweep returns
    /// `Ok`: the in-sweep flush retracts the leg rows of the entries, the
    /// ledger step the rest. Equals what `legs_retracted_total` counts.
    pub(super) retracted: u64,
    /// Buffer entries that existed and were persisted.
    pub(super) entries: usize,
}

/// Retracts every provisional ledger row of `chain_id` at or below `frontier`
/// that is outside every open failed range (ADR-022, ADR-005). `frontier` is
/// the end of the last confirmed realtime range whose failures are recorded,
/// never the RPC tag head.
///
/// Order is the durability rule: the buffer step persists the retracted entries,
/// then one maintenance cycle flushes them (the canonical row and the entries'
/// leg rows, in one transaction), and only then are the ledger rows marked
/// `retracted`. Cold entries are restored only on demand and the sweep no longer
/// selects a row once it is `retracted`, so without the flush a stop after the
/// ledger step would leave the canonical row showing the retracted leg forever.
/// A failure before the ledger step leaves the rows provisional and the next
/// sweep repeats.
pub(super) async fn sweep(ctx: &RunContext, chain_id: i64, frontier: u64) -> Result<SweepOutcome> {
    let rows = select(ctx, chain_id, frontier).await?;
    if rows.is_empty() {
        return Ok(SweepOutcome::default());
    }
    let entries = retract_in_buffer(ctx, &rows).await?;
    if entries > 0 {
        ctx.buffer
            .run()
            .await
            .context("failed to flush the retracted OP interop entries")?;
    }
    retract_in_ledger(ctx, chain_id, &rows).await?;

    let mut by_kind: HashMap<&str, u64> = HashMap::new();
    for row in &rows {
        *by_kind.entry(row.event_kind.as_str()).or_default() += 1;
    }
    for (event_kind, count) in by_kind {
        metrics::OP_INTEROP_LEGS_RETRACTED_TOTAL
            .with_label_values(&[
                ctx.bridge_id.to_string().as_str(),
                chain_id.to_string().as_str(),
                event_kind,
            ])
            .inc_by(count);
    }
    Ok(SweepOutcome {
        retracted: rows.len() as u64,
        entries,
    })
}

/// Step 1: the provisional rows at or below `frontier`.
pub(super) async fn select(
    ctx: &RunContext,
    chain_id: i64,
    frontier: u64,
) -> Result<Vec<ProvisionalEvent>> {
    let frontier = i64::try_from(frontier).context("sweep frontier does not fit i64")?;
    provisional_op_interop_events(
        ctx.db.db.as_ref(),
        ctx.bridge_id,
        chain_id,
        frontier,
        SWEEP_BATCH,
    )
    .await
    .with_context(|| {
        format!(
            "failed to select provisional OP interop rows (bridge {}, chain {chain_id})",
            ctx.bridge_id
        )
    })
}

/// Step 2: the entries of the selected `sent_message` / `relayed_message`
/// rows that carry a `message_id`, retracted and persisted by
/// `alter_existing_durably`. Executing rows follow their relay (same block);
/// raw rows have no entry. Returns the entries that existed.
pub(super) async fn retract_in_buffer(
    ctx: &RunContext,
    rows: &[ProvisionalEvent],
) -> Result<usize> {
    let bridge_id = i16::try_from(ctx.bridge_id).context("bridge_id out of range")?;
    let leg_kinds = [
        OpInteropEvent::SentMessage.as_str(),
        OpInteropEvent::RelayedMessage.as_str(),
    ];
    let mut keys: Vec<Key> = Vec::new();
    let mut logs: Vec<(B256, u64)> = Vec::new();
    for row in rows {
        let Some(message_id) = row.message_id else {
            continue;
        };
        if !leg_kinds.contains(&row.event_kind.as_str()) {
            continue;
        }
        let (Ok(block_hash), Ok(log_index)) = (
            B256::try_from(row.block_hash.as_slice()),
            u64::try_from(row.log_index),
        ) else {
            continue;
        };
        let key = Key::new(message_id, bridge_id);
        if !keys.contains(&key) {
            keys.push(key);
        }
        logs.push((block_hash, log_index));
    }
    if keys.is_empty() {
        return Ok(0);
    }
    // A log identifies itself uniquely and an entry holds only its own logs,
    // so retracting the union in every entry is exact.
    let existing = ctx
        .buffer
        .alter_existing_durably(&keys, |message: &mut Message| message.retract(&logs))
        .await
        .context("failed to retract OP interop legs in the buffer")?;
    Ok(existing.len())
}

/// Step 3: `retract_op_interop_events` for every selected row. Returns the rows
/// this step changed; the in-sweep flush may already have retracted the rest.
pub(super) async fn retract_in_ledger(
    ctx: &RunContext,
    chain_id: i64,
    rows: &[ProvisionalEvent],
) -> Result<u64> {
    let keys: Vec<(Vec<u8>, i64)> = rows
        .iter()
        .map(|row| (row.block_hash.clone(), row.log_index))
        .collect();
    retract_op_interop_events(ctx.db.db.as_ref(), ctx.bridge_id, chain_id, &keys)
        .await
        .with_context(|| {
            format!(
                "failed to retract provisional OP interop rows (bridge {}, chain {chain_id})",
                ctx.bridge_id
            )
        })
}
