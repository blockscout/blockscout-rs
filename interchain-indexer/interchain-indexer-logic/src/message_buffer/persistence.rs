// SPDX-License-Identifier: LicenseRef-Blockscout

use std::collections::HashMap;

use alloy::primitives::ChainId;
use interchain_indexer_entity::{
    amb_message_anomalies, amb_messages_confirmations, crosschain_messages, crosschain_transfers,
    indexer_checkpoints, pending_messages,
};
use itertools::Itertools;
use sea_orm::{
    ActiveValue, ColumnTrait, DatabaseTransaction, DbErr, EntityTrait, QueryFilter,
    sea_query::{Expr, OnConflict},
};
use std::collections::HashSet;

use super::{BufferItem, Consolidate, ConsolidatedMessage, DestinationExecution, Key};
use crate::{
    bulk::{self, batched_upsert, run_in_chunks},
    message_buffer::cursor::{BridgeId, Cursor, CursorBlocksBuilder, Cursors},
    protocol_metadata::{
        AdditionalExecution, MultipleExecutions, MultipleExecutionsProtocol, ProtocolMetadata,
        XDaiMultipleExecutions, rfc3339_millis_z,
    },
    stats::metrics::STATS_TRANSFER_ASSET_LINKAGE_UNSET_TOTAL,
};

fn pending_messages_on_conflict() -> OnConflict {
    OnConflict::columns([
        pending_messages::Column::MessageId,
        pending_messages::Column::BridgeId,
    ])
    .update_column(pending_messages::Column::Payload)
    .to_owned()
}

/// Conflict policy for `crosschain_messages`: a **non-regressing merge**.
///
/// The two halves of a message (source and destination) are produced by
/// independently scanned chains, so they can be flushed out of order. A
/// destination execution finalizes the row (`Completed`/`Failed`) and evicts the
/// buffer entry (hot + cold); if the source side is then observed it rebuilds a
/// *source-only* entry under the same PK and flushes it as `Initiated`/
/// `ReadyToClaim`. A blind `update_columns` overwrite (the previous behaviour)
/// would regress the terminal status and drop the destination tx hash.
///
/// Instead we merge, so the outcome is order-independent:
/// - **status** never regresses out of a terminal state — if the stored row is
///   `completed`/`failed` and the incoming one is not, the stored status wins;
/// - **destination identity** (`dst_*`, recipient) is preserved while keeping a
///   terminal status, and otherwise prefers the incoming value without nulling;
/// - **source/other columns** are filled from whichever side carries them
///   (`COALESCE`), `init_timestamp` keeps the earliest and `last_update_timestamp`
///   the latest.
///
/// `EXCLUDED` is the incoming row; `crosschain_messages` is the stored row.
fn crosschain_messages_on_conflict() -> OnConflict {
    const KEEP_TERMINAL: &str = "crosschain_messages.status::text IN ('completed', 'failed') \
         AND EXCLUDED.status::text NOT IN ('completed', 'failed')";

    // Destination-owned column: keep the stored value when we are keeping a
    // terminal status, otherwise prefer the incoming value but never null it out.
    let keep_existing_if_terminal = |col: &str| {
        Expr::cust(format!(
            "CASE WHEN {KEEP_TERMINAL} \
             THEN crosschain_messages.{col} \
             ELSE COALESCE(EXCLUDED.{col}, crosschain_messages.{col}) END"
        ))
    };
    // Source-owned / neutral column: take the incoming value, falling back to the
    // stored one when the incoming side does not carry it.
    let prefer_incoming = |col: &str| {
        Expr::cust(format!(
            "COALESCE(EXCLUDED.{col}, crosschain_messages.{col})"
        ))
    };

    // Single source of truth for the resulting `dst_chain_id`, reused as
    // both the column value and the condition the `protocol_metadata` rule
    // below branches on — duplicating this SQL would risk the two rules
    // silently disagreeing on whether the destination is known.
    let dst_result_sql = format!(
        "CASE WHEN {KEEP_TERMINAL} \
         THEN crosschain_messages.dst_chain_id \
         ELSE COALESCE(EXCLUDED.dst_chain_id, crosschain_messages.dst_chain_id) END"
    );

    // `unresolved_destination` diagnostics only make sense while the
    // destination is unknown: as soon as the merged `dst_chain_id` is known,
    // drop that namespace (merging can otherwise resurrect a stale snapshot
    // from a late `Unresolved` observation, or leave a cleared row's
    // diagnostics untouched). Any other namespace merges via plain `||` and
    // is never removed — this rule is specific to `unresolved_destination`
    // because that concept, by definition, loses meaning once identity is
    // known. `NULLIF(..., '{{}}'::jsonb)` turns an empty merge result back
    // into SQL NULL: an empty JSON object must never reach the column.
    let protocol_metadata_sql = format!(
        "NULLIF(\
         CASE WHEN ({dst_result_sql}) IS NOT NULL \
              THEN (COALESCE(crosschain_messages.protocol_metadata, '{{}}'::jsonb) \
                    || COALESCE(EXCLUDED.protocol_metadata, '{{}}'::jsonb)) \
                   - 'unresolved_destination' \
              ELSE (COALESCE(crosschain_messages.protocol_metadata, '{{}}'::jsonb) \
                    || COALESCE(EXCLUDED.protocol_metadata, '{{}}'::jsonb)) \
         END, \
         '{{}}'::jsonb)"
    );

    OnConflict::columns([
        crosschain_messages::Column::Id,
        crosschain_messages::Column::BridgeId,
    ])
    .value(
        crosschain_messages::Column::Status,
        Expr::cust(format!(
            "CASE WHEN {KEEP_TERMINAL} \
             THEN crosschain_messages.status ELSE EXCLUDED.status END"
        )),
    )
    .value(
        crosschain_messages::Column::InitTimestamp,
        Expr::cust("LEAST(crosschain_messages.init_timestamp, EXCLUDED.init_timestamp)"),
    )
    .value(
        crosschain_messages::Column::LastUpdateTimestamp,
        Expr::cust(
            "GREATEST(crosschain_messages.last_update_timestamp, EXCLUDED.last_update_timestamp)",
        ),
    )
    .value(
        crosschain_messages::Column::SrcChainId,
        prefer_incoming("src_chain_id"),
    )
    .value(
        crosschain_messages::Column::DstChainId,
        Expr::cust(dst_result_sql),
    )
    .value(
        crosschain_messages::Column::SrcTxHash,
        prefer_incoming("src_tx_hash"),
    )
    .value(
        crosschain_messages::Column::DstTxHash,
        keep_existing_if_terminal("dst_tx_hash"),
    )
    .value(
        crosschain_messages::Column::SenderAddress,
        prefer_incoming("sender_address"),
    )
    .value(
        crosschain_messages::Column::RecipientAddress,
        keep_existing_if_terminal("recipient_address"),
    )
    .value(
        crosschain_messages::Column::Payload,
        prefer_incoming("payload"),
    )
    .value(
        crosschain_messages::Column::ProtocolMetadata,
        Expr::cust(protocol_metadata_sql),
    )
    .to_owned()
}

fn crosschain_transfers_on_conflict() -> OnConflict {
    // Transfer sides are reconstructed independently. A later partial flush must
    // enrich the missing side without clearing the side that was already known.
    let prefer_incoming = |col: &str| {
        Expr::cust(format!(
            "COALESCE(EXCLUDED.{col}, crosschain_transfers.{col})"
        ))
    };

    OnConflict::columns([
        crosschain_transfers::Column::MessageId,
        crosschain_transfers::Column::BridgeId,
        crosschain_transfers::Column::Index,
    ])
    .value(
        crosschain_transfers::Column::TokenSrcChainId,
        Expr::cust("EXCLUDED.token_src_chain_id"),
    )
    .value(
        crosschain_transfers::Column::TokenDstChainId,
        Expr::cust("EXCLUDED.token_dst_chain_id"),
    )
    .value(
        crosschain_transfers::Column::SrcAmount,
        prefer_incoming("src_amount"),
    )
    .value(
        crosschain_transfers::Column::DstAmount,
        prefer_incoming("dst_amount"),
    )
    .value(
        crosschain_transfers::Column::TokenSrcAddress,
        prefer_incoming("token_src_address"),
    )
    .value(
        crosschain_transfers::Column::TokenDstAddress,
        prefer_incoming("token_dst_address"),
    )
    .value(
        crosschain_transfers::Column::SenderAddress,
        prefer_incoming("sender_address"),
    )
    .value(
        crosschain_transfers::Column::RecipientAddress,
        prefer_incoming("recipient_address"),
    )
    .value(
        crosschain_transfers::Column::TokenIds,
        prefer_incoming("token_ids"),
    )
    // Write-once, and deliberately the reverse of `prefer_incoming` above:
    // once an indexer has declared a linkage, a later flush can never change
    // it. `NULL -> value` still applies (an indexer that only learns the
    // linkage on a later flush can still state it); `value -> different value`
    // is silently dropped. `src_stats_asset_id` / `dst_stats_asset_id` are
    // deliberately absent from this OnConflict: they are projection-owned,
    // exactly as `stats_asset_id` was, and a flush must never clobber them.
    .value(
        crosschain_transfers::Column::AssetLinkage,
        Expr::cust("COALESCE(crosschain_transfers.asset_linkage, EXCLUDED.asset_linkage)"),
    )
    .value(
        crosschain_transfers::Column::UpdatedAt,
        Expr::current_timestamp(),
    )
    .to_owned()
}

fn amb_messages_confirmations_on_conflict() -> OnConflict {
    OnConflict::columns([
        amb_messages_confirmations::Column::MessageId,
        amb_messages_confirmations::Column::BridgeId,
        amb_messages_confirmations::Column::ValidatorAddress,
    ])
    .do_nothing()
    .to_owned()
}

// Anomalies are append-only investigative rows with a DB-assigned BIGSERIAL PK,
// so there is no natural conflict key. `batched_upsert` requires an `OnConflict`;
// a no-target `do_nothing` is the no-op form (rows are inserted, never updated).
fn amb_message_anomalies_on_conflict() -> OnConflict {
    OnConflict::new().do_nothing().to_owned()
}

fn consolidated_message_pk(
    message: &crosschain_messages::ActiveModel,
) -> Result<(i64, i32), DbErr> {
    match (&message.id, &message.bridge_id) {
        (ActiveValue::Set(id), ActiveValue::Set(bridge_id)) => Ok((*id, *bridge_id)),
        _ => Err(DbErr::Custom(
            "consolidated message must have id and bridge_id set".into(),
        )),
    }
}

async fn delete_replaced_messages(
    tx: &DatabaseTransaction,
    replacement_pks: &[(i64, i32)],
) -> Result<(), DbErr> {
    let keys: Vec<(i64, i32)> = replacement_pks
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    // Row-valued `IN`: size by `ROW_IN_KEY_CHUNK`, not by bind width — see
    // `bulk::ROW_IN_KEY_CHUNK`.
    run_in_chunks(&keys, bulk::ROW_IN_KEY_CHUNK, |batch| async {
        crosschain_messages::Entity::delete_many()
            .filter(
                Expr::tuple([
                    Expr::col(crosschain_messages::Column::Id).into(),
                    Expr::col(crosschain_messages::Column::BridgeId).into(),
                ])
                .in_tuples(batch.iter().copied()),
            )
            .exec(tx)
            .await
            .map(|_| ())
    })
    .await
}

pub(super) async fn offload_stale_to_pending<T: Consolidate>(
    tx: &DatabaseTransaction,
    stale_entries: &[(Key, BufferItem<T>)],
) -> Result<(), DbErr> {
    let models: Vec<pending_messages::ActiveModel> = stale_entries
        .iter()
        .map(|(key, entry)| {
            let payload = serde_json::to_value(entry).map_err(|e| {
                DbErr::Custom(format!("pending_messages payload serialize failed: {e}"))
            })?;
            Ok(pending_messages::ActiveModel {
                message_id: ActiveValue::Set(key.message_id),
                bridge_id: ActiveValue::Set(key.bridge_id as i32),
                payload: ActiveValue::Set(payload),
                created_at: ActiveValue::Set(Some(entry.hot_since)),
            })
        })
        .collect::<Result<_, DbErr>>()?;

    batched_upsert(tx, &models, pending_messages_on_conflict()).await
}

/// The single production write chokepoint for `crosschain_transfers` is this
/// module's `flush_to_final_storage` — every indexer's transfers reach the
/// database through it. Every transfer constructor is expected to start from
/// `interchain_indexer_entity::new_transfer`, which always sets
/// `asset_linkage`; an `ActiveValue::NotSet` here means some constructor built
/// the row a different way (most likely `..Default::default()`, whose
/// `ActiveModel::default()` leaves every field `NotSet`).
///
/// A `NotSet` column is omitted from the INSERT column list, so
/// `EXCLUDED.asset_linkage` would be the column default (`NULL`), and the
/// write-once `COALESCE` in `crosschain_transfers_on_conflict` would then keep
/// `NULL` forever -- silently deferring every transfer from that indexer. Do
/// not substitute a guessed default here: a wrong `mirror` is the one
/// irreversible direction (it unions two assets), which is exactly what the
/// deferral design exists to prevent. Leave the value unset so the row defers
/// like any other unclassified transfer.
fn reject_unset_asset_linkage(transfers: &[crosschain_transfers::ActiveModel]) {
    for transfer in transfers {
        if matches!(transfer.asset_linkage, ActiveValue::NotSet) {
            debug_assert!(
                false,
                "crosschain_transfers.asset_linkage must be Set by every transfer \
                 constructor; found NotSet for bridge_id={:?} message_id={:?}",
                transfer.bridge_id, transfer.message_id
            );
            tracing::warn!(
                bridge_id = ?transfer.bridge_id,
                message_id = ?transfer.message_id,
                "stats projection: transfer reached flush_to_final_storage with \
                 asset_linkage NotSet; leaving it unset so the row defers instead \
                 of guessing"
            );
            STATS_TRANSFER_ASSET_LINKAGE_UNSET_TOTAL.inc();
        }
    }
}

pub(super) async fn flush_to_final_storage(
    tx: &DatabaseTransaction,
    consolidated_entries: Vec<ConsolidatedMessage>,
) -> Result<(), DbErr> {
    let replacement_pks = consolidated_entries
        .iter()
        .filter(|entry| entry.replace_existing)
        .map(|entry| consolidated_message_pk(&entry.message))
        .collect::<Result<Vec<_>, _>>()?;

    let (messages, transfers, amb_confirmations, amb_anomalies): (Vec<_>, Vec<_>, Vec<_>, Vec<_>) =
        consolidated_entries
            .into_iter()
            .map(|c| (c.message, c.transfers, c.amb_confirmations, c.amb_anomalies))
            .multiunzip();
    let transfers = transfers.into_iter().flatten().collect::<Vec<_>>();
    let amb_confirmations = amb_confirmations.into_iter().flatten().collect::<Vec<_>>();
    let amb_anomalies = amb_anomalies.into_iter().flatten().collect::<Vec<_>>();

    reject_unset_asset_linkage(&transfers);

    delete_replaced_messages(tx, &replacement_pks).await?;
    batched_upsert(tx, &messages, crosschain_messages_on_conflict()).await?;
    batched_upsert(tx, &transfers, crosschain_transfers_on_conflict()).await?;
    batched_upsert(
        tx,
        &amb_confirmations,
        amb_messages_confirmations_on_conflict(),
    )
    .await?;
    batched_upsert(tx, &amb_anomalies, amb_message_anomalies_on_conflict()).await?;

    Ok(())
}

/// Result of [`reconcile_destination_executions`]: what to write once
/// `flush_to_final_storage` has run, plus which keys have nothing left to
/// wait for.
///
/// Currently xDai-only in practice: [`Consolidate::destination_executions`]
/// defaults to empty, so AMB and Avalanche never populate `observations` and
/// this type is always empty for them.
pub(super) struct DestinationExecutionReconciliation {
    /// Keys with nothing left to wait for: safe to clear from
    /// `pending_messages` and evict from the hot tier once this reconciliation
    /// (and the flush it straddles) has committed.
    pub(super) resolved_keys: Vec<Key>,
    /// Anomaly rows selected for insertion, before the storage-level dedup
    /// `apply_destination_execution_reconciliation` performs against
    /// `amb_message_anomalies` itself.
    promoted: Vec<amb_message_anomalies::ActiveModel>,
    /// Full per-key `{"multiple_executions": {...}}` namespace values, one
    /// per key that gained at least one new candidate this cycle, ready to
    /// `||`-merge into `crosschain_messages.protocol_metadata` as is. Never
    /// partial: `||` replaces the whole namespace key, so a partial value
    /// here would silently drop previously recorded executions.
    metadata_patches: Vec<((i64, i32), serde_json::Value)>,
}

impl DestinationExecutionReconciliation {
    fn empty() -> Self {
        Self {
            resolved_keys: Vec::new(),
            promoted: Vec::new(),
            metadata_patches: Vec::new(),
        }
    }
}

/// The `crosschain_messages` columns relevant to destination-execution
/// reconciliation, read once per key before any write decision is made.
struct StoredDestinationState {
    dst_tx_hash: Option<Vec<u8>>,
    recipient_address: Option<Vec<u8>>,
    last_update_timestamp: Option<chrono::NaiveDateTime>,
    protocol_metadata: Option<serde_json::Value>,
}

/// Reads the stored destination state for every key with at least one
/// observation, decides -- per key -- whether a canonical row exists or is
/// about to be written this same transaction (the *promotion gate*), and:
///
/// - when the **stored** execution is the one that wins, restores
///   `consolidated_entries`' destination-owned message fields to the stored
///   values and drops that entry's transfers (so the upsert in
///   `flush_to_final_storage` cannot clobber `dst_tx_hash` /
///   `recipient_address` / `last_update_timestamp`, or any column of the
///   already-stored canonical transfer, with a different, non-canonical
///   execution this buffer instance happened to see first);
/// - collects anomaly-row candidates and the full replacement
///   `multiple_executions` array for keys that gained a new one;
/// - reports which keys are fully resolved (nothing left to wait for), for
///   the caller to fold into pending cleanup and hot eviction.
///
/// Returns immediately, with zero queries, when `observations` is empty --
/// Hard Constraint 3 (AMB/Avalanche's path must stay bit-for-bit unchanged).
pub(super) async fn reconcile_destination_executions(
    tx: &DatabaseTransaction,
    consolidated_entries: &mut [ConsolidatedMessage],
    observations: &[(Key, Vec<DestinationExecution>)],
) -> Result<DestinationExecutionReconciliation, DbErr> {
    if observations.is_empty() {
        return Ok(DestinationExecutionReconciliation::empty());
    }

    let keys: Vec<(i64, i32)> = observations
        .iter()
        .map(|(key, _)| (key.message_id, key.bridge_id as i32))
        .collect();

    // Row-valued `IN`: chunk by `ROW_IN_KEY_CHUNK`, not by bind width, and
    // hand-roll the accumulator loop -- `run_in_batches`'s closure cannot lend
    // out a mutable accumulator (`.memory-bank/rules/database.md`).
    let mut stored_by_pk: HashMap<(i64, i32), StoredDestinationState> = HashMap::new();
    for batch in keys.chunks(bulk::ROW_IN_KEY_CHUNK) {
        let rows = crosschain_messages::Entity::find()
            .filter(
                Expr::tuple([
                    Expr::col(crosschain_messages::Column::Id).into(),
                    Expr::col(crosschain_messages::Column::BridgeId).into(),
                ])
                .in_tuples(batch.iter().copied()),
            )
            .all(tx)
            .await?;
        for row in rows {
            stored_by_pk.insert(
                (row.id, row.bridge_id),
                StoredDestinationState {
                    dst_tx_hash: row.dst_tx_hash,
                    recipient_address: row.recipient_address,
                    last_update_timestamp: row.last_update_timestamp,
                    protocol_metadata: row.protocol_metadata,
                },
            );
        }
    }

    let index_by_pk: HashMap<(i64, i32), usize> = consolidated_entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            consolidated_message_pk(&entry.message)
                .ok()
                .map(|pk| (pk, index))
        })
        .collect();

    let mut reconciliation = DestinationExecutionReconciliation::empty();

    for (key, key_observations) in observations {
        if key_observations.is_empty() {
            continue;
        }
        debug_assert!(
            key_observations
                .iter()
                .all(|observation| observation.key == *key),
            "DestinationExecution::key must match the key it was grouped under"
        );
        let pk = (key.message_id, key.bridge_id as i32);
        let stored = stored_by_pk.get(&pk);
        let consolidated_index = index_by_pk.get(&pk).copied();

        // The stored `dst_tx_hash` wins when present; otherwise the first
        // observation in processing order is canonical by construction (it is
        // the same execution `consolidate()` used to build this cycle's
        // `ConsolidatedMessage`, when there is one).
        let canonical_tx: Vec<u8> = stored
            .and_then(|s| s.dst_tx_hash.clone())
            .unwrap_or_else(|| key_observations[0].tx_hash.clone());
        let stored_wins = stored.is_some_and(|s| s.dst_tx_hash.is_some());

        // Promotion gate: a candidate can only be promoted once there is
        // somewhere for it to point -- either the canonical row already
        // exists **with a destination transaction to point at**, or this same
        // transaction is about to write one. A stored row alone is not
        // enough: an `Initiated` row (source flushed, destination still
        // unresolved) has a PK but no `dst_tx_hash`, so `stored.is_some()`
        // must not make the key resolved on its own -- the anomalies table
        // has no FK, so a row whose `buffer_key` points at nothing would
        // insert silently, and declaring the key resolved would also clear
        // `pending_messages` and evict it from hot with the canonical
        // execution still unwritten.
        let promotable = stored_wins || consolidated_index.is_some();

        let mut seen_tx_hashes: HashSet<Vec<u8>> = HashSet::new();
        let candidates: Vec<&DestinationExecution> = key_observations
            .iter()
            .filter(|observation| observation.tx_hash != canonical_tx)
            .filter(|observation| seen_tx_hashes.insert(observation.tx_hash.clone()))
            .collect();

        if !promotable {
            // Nothing to write against yet. Leave the observations buffered
            // and keep re-checking on later cycles -- see the comment on this
            // exact "never resolves without a source" case in
            // `message_buffer/maintenance.rs`.
            continue;
        }

        reconciliation.resolved_keys.push(*key);

        if stored_wins && let Some(index) = consolidated_index {
            // The stored row was written together with its canonical
            // transfer by the flush that first finalized it (xDai, the only
            // producer of destination executions, always emits its single
            // `index = 0` transfer alongside the message), so there is
            // nothing this cycle's (possibly non-canonical) transfer can add
            // -- only things it can corrupt. Its transfers are therefore
            // dropped outright rather than neutralized column by column:
            // `crosschain_transfers_on_conflict` is `COALESCE(EXCLUDED, stored)`
            // for every value column (and unconditional `EXCLUDED` for the
            // token chain ids), so *any* completion-derived column left `Set`
            // would overwrite the canonical value. That is not hypothetical:
            // for a raw-hash identity whose source receipt carries no
            // recognized source event, xDai takes both `recipient_address`
            // and `src_amount` from the completion itself, and a per-column
            // denylist here would silently rot the next time a column is
            // derived from the destination side. Stats projection is
            // unaffected: `apply_stats_for_flushed_batch` re-reads the stored
            // transfer by message PK, and the upsert never touches
            // `stats_processed` / `*_stats_asset_id` anyway, so the already
            // counted row is neither re-counted nor re-linked from late data.
            //
            // The message row cannot be dropped the same way -- it is what
            // this cycle's confirmations and `protocol_metadata` patch attach
            // to -- so its destination-owned columns are restored to the
            // stored values instead. Its remaining columns are source-owned
            // and shared with the canonical execution by construction.
            let stored = stored.expect("stored_wins implies stored is Some");
            let entry = &mut consolidated_entries[index];
            entry.message.dst_tx_hash = ActiveValue::Set(stored.dst_tx_hash.clone());
            entry.message.recipient_address = ActiveValue::Set(stored.recipient_address.clone());
            entry.message.last_update_timestamp = ActiveValue::Set(stored.last_update_timestamp);
            entry.transfers.clear();
        }

        if candidates.is_empty() {
            continue;
        }

        let canonical_executor: Option<Vec<u8>> = stored
            .and_then(|s| s.recipient_address.clone())
            .or_else(|| key_observations[0].executor.clone());

        let mut additional_executions: Vec<AdditionalExecution> =
            ProtocolMetadata::from_json_value(stored.and_then(|s| s.protocol_metadata.clone()))
                .and_then(|metadata| metadata.multiple_executions)
                .map(|multiple| multiple.additional_executions)
                .unwrap_or_default();
        let mut known_hashes: HashSet<String> = additional_executions
            .iter()
            .map(|execution| execution.transaction_hash.clone())
            .collect();

        // Whether any candidate actually grew the stored array this cycle --
        // gates `metadata_patches` below so re-processing a candidate whose
        // hash is already in the stored array (a replayed late execution)
        // does not queue an idempotent no-op `UPDATE` on every cycle.
        let mut array_changed = false;

        for candidate in &candidates {
            let transaction_hash = alloy::hex::encode_prefixed(&candidate.tx_hash);
            if known_hashes.insert(transaction_hash.clone()) {
                additional_executions.push(AdditionalExecution {
                    transaction_hash,
                    timestamp: rfc3339_millis_z(candidate.block_timestamp),
                });
                array_changed = true;
            }

            reconciliation
                .promoted
                .push(amb_message_anomalies::ActiveModel {
                    id: ActiveValue::NotSet,
                    bridge_id: ActiveValue::Set(key.bridge_id as i32),
                    buffer_key: ActiveValue::Set(key.message_id),
                    native_id: ActiveValue::Set(candidate.native_id.clone()),
                    event_kind: ActiveValue::Set("destination_execution".to_string()),
                    chain_id: ActiveValue::Set(candidate.chain_id),
                    tx_hash: ActiveValue::Set(candidate.tx_hash.clone()),
                    log_index: ActiveValue::Set(candidate.log_index),
                    block_number: ActiveValue::Set(candidate.block_number),
                    block_timestamp: ActiveValue::Set(candidate.block_timestamp),
                    sender: ActiveValue::Set(None),
                    executor: ActiveValue::Set(candidate.executor.clone()),
                    src_chain_id: ActiveValue::Set(candidate.src_chain_id),
                    dst_chain_id: ActiveValue::Set(candidate.dst_chain_id),
                    encoded_data: ActiveValue::Set(None),
                    conflict_sender: ActiveValue::Set(None),
                    conflict_executor: ActiveValue::Set(canonical_executor.clone()),
                    conflict_tx_hash: ActiveValue::Set(Some(canonical_tx.clone())),
                    detail: ActiveValue::Set(Some(candidate.detail.clone())),
                    created_at: ActiveValue::NotSet,
                });
        }

        if !array_changed {
            // Every candidate this cycle was already in the stored array
            // (e.g. a replayed late execution): the anomaly rows above still
            // go through storage-level dedup in
            // `apply_destination_execution_reconciliation`, but there is
            // nothing new for `protocol_metadata` to record, so skip queuing
            // an otherwise idempotent `UPDATE`.
            continue;
        }

        // Namespace-only patch: `ProtocolMetadata`'s `skip_serializing_if` on
        // both fields means a struct with only `multiple_executions` set
        // serializes to exactly `{"multiple_executions": {...}}`, never
        // touching `unresolved_destination` -- the `||` merge at apply time
        // leaves that key, if present, untouched.
        let namespace = ProtocolMetadata {
            multiple_executions: Some(MultipleExecutions {
                additional_executions,
                protocol: MultipleExecutionsProtocol::XDai(XDaiMultipleExecutions {}),
            }),
            ..Default::default()
        }
        .to_json_value()
        .expect("a populated multiple_executions namespace always serializes to Some");

        reconciliation.metadata_patches.push((pk, namespace));
    }

    Ok(reconciliation)
}

fn amb_message_anomaly_dedup_tuple(
    anomaly: &amb_message_anomalies::ActiveModel,
) -> Option<(i32, i64, i64, Vec<u8>)> {
    match (
        &anomaly.bridge_id,
        &anomaly.buffer_key,
        &anomaly.chain_id,
        &anomaly.tx_hash,
    ) {
        (
            ActiveValue::Set(bridge_id),
            ActiveValue::Set(buffer_key),
            ActiveValue::Set(chain_id),
            ActiveValue::Set(tx_hash),
        ) => Some((*bridge_id, *buffer_key, *chain_id, tx_hash.clone())),
        _ => None,
    }
}

/// Applies a [`DestinationExecutionReconciliation`] built by
/// [`reconcile_destination_executions`]. Must run **after**
/// `flush_to_final_storage`, so a row written by this same transaction
/// already exists when the anomaly rows and metadata patches reference it.
///
/// Returns immediately, with zero queries, when there is nothing to promote
/// and no metadata to patch.
pub(super) async fn apply_destination_execution_reconciliation(
    tx: &DatabaseTransaction,
    reconciliation: &DestinationExecutionReconciliation,
) -> Result<(), DbErr> {
    if reconciliation.promoted.is_empty() && reconciliation.metadata_patches.is_empty() {
        return Ok(());
    }

    // Dedup against the anomalies table itself: it has no natural conflict
    // key (BIGSERIAL PK; `amb_message_anomalies_on_conflict()` is a
    // no-target `do_nothing()`), so re-running this same reconciliation on a
    // later cycle -- which happens whenever the buffer item is touched again,
    // even by an idempotent replay that bumps its version -- must not insert
    // a second physical row for the same logical anomaly. Keyed on
    // `(bridge_id, buffer_key, chain_id, tx_hash)` rather than `native_id`:
    // the table's only index is `(bridge_id, native_id)`, so this scan does
    // not use it, but the table is tiny and adding an index is not an option
    // (the schema is frozen).
    let candidate_tuples: Vec<(i32, i64, i64, Vec<u8>)> = reconciliation
        .promoted
        .iter()
        .filter_map(amb_message_anomaly_dedup_tuple)
        .collect();

    // Keyed on the full dedup tuple, not `tx_hash` alone: the SELECT above
    // filters on `(bridge_id, buffer_key, chain_id, tx_hash)`, and collapsing
    // the result to `tx_hash` would let a stored anomaly for one key suppress
    // a legitimate candidate for a different key that happens to share a
    // `tx_hash` (e.g. two different `buffer_key`s observing the same
    // destination transaction).
    let mut already_stored: HashSet<(i32, i64, i64, Vec<u8>)> = HashSet::new();
    for batch in candidate_tuples.chunks(bulk::ROW_IN_KEY_CHUNK) {
        let rows = amb_message_anomalies::Entity::find()
            .filter(amb_message_anomalies::Column::EventKind.eq("destination_execution"))
            .filter(
                Expr::tuple([
                    Expr::col(amb_message_anomalies::Column::BridgeId).into(),
                    Expr::col(amb_message_anomalies::Column::BufferKey).into(),
                    Expr::col(amb_message_anomalies::Column::ChainId).into(),
                    Expr::col(amb_message_anomalies::Column::TxHash).into(),
                ])
                .in_tuples(batch.iter().cloned()),
            )
            .all(tx)
            .await?;
        already_stored.extend(
            rows.into_iter()
                .map(|row| (row.bridge_id, row.buffer_key, row.chain_id, row.tx_hash)),
        );
    }

    let rows: Vec<amb_message_anomalies::ActiveModel> = reconciliation
        .promoted
        .iter()
        .filter(|anomaly| match amb_message_anomaly_dedup_tuple(anomaly) {
            Some(dedup_tuple) => !already_stored.contains(&dedup_tuple),
            None => true,
        })
        .cloned()
        .collect();

    batched_upsert(tx, &rows, amb_message_anomalies_on_conflict()).await?;

    // The array is always written whole: `||` replaces the `multiple_executions`
    // key entirely, so `reconcile_destination_executions` assembled the full
    // replacement value (stored + new) already -- nothing is patched
    // in SQL here.
    for ((id, bridge_id), namespace) in &reconciliation.metadata_patches {
        crosschain_messages::Entity::update_many()
            .col_expr(
                crosschain_messages::Column::ProtocolMetadata,
                Expr::cust_with_values(
                    "COALESCE(protocol_metadata, '{}'::jsonb) || $1::jsonb",
                    [namespace.clone()],
                ),
            )
            .filter(crosschain_messages::Column::Id.eq(*id))
            .filter(crosschain_messages::Column::BridgeId.eq(*bridge_id))
            .exec(tx)
            .await?;
    }

    Ok(())
}

/// Distinct `(chain_id, token_address)` from flushed transfers for async token
/// enrichment. Covers **all** flushed entries, not only finalized ones: a
/// transfer to a chain unindexed for its bridge is never `is_final` (the
/// destination-side events can never arrive) but is now countable and
/// asset-linked, so its known-side token must still be eligible for
/// enrichment.
pub(super) fn token_keys_from_flushed_for_enrichment(
    flushed: &[ConsolidatedMessage],
) -> Vec<(i64, Vec<u8>)> {
    let mut out = HashSet::new();
    for c in flushed {
        for t in &c.transfers {
            if let (ActiveValue::Set(sc), ActiveValue::Set(Some(sa))) =
                (&t.token_src_chain_id, &t.token_src_address)
            {
                out.insert((*sc, sa.clone()));
            }
            if let (ActiveValue::Set(dc), ActiveValue::Set(Some(da))) =
                (&t.token_dst_chain_id, &t.token_dst_address)
            {
                out.insert((*dc, da.clone()));
            }
        }
    }
    out.into_iter().collect()
}

pub(super) async fn remove_finalized_from_pending(
    tx: &DatabaseTransaction,
    keys_to_remove_from_pending: &[Key],
) -> Result<(), DbErr> {
    let keys: Vec<(i64, i32)> = keys_to_remove_from_pending
        .iter()
        .map(|k| (k.message_id, k.bridge_id as i32))
        .collect();

    // Row-valued `IN`: size by `ROW_IN_KEY_CHUNK`, not by bind width — see
    // `bulk::ROW_IN_KEY_CHUNK`. This is the statement that kept overflowing the
    // planner stack at the old `PG_BIND_PARAM_LIMIT / 2` sizing.
    run_in_chunks(&keys, bulk::ROW_IN_KEY_CHUNK, |batch| async {
        pending_messages::Entity::delete_many()
            .filter(
                Expr::tuple([
                    Expr::col(pending_messages::Column::MessageId).into(),
                    Expr::col(pending_messages::Column::BridgeId).into(),
                ])
                .in_tuples(batch.iter().copied()),
            )
            .exec(tx)
            .await
            .map(|_| ())
    })
    .await
}

pub(super) async fn fetch_cursors(
    cursor_builder: &CursorBlocksBuilder,
    tx: &DatabaseTransaction,
) -> Result<Cursors, DbErr> {
    let tuples = cursor_builder
        .inner
        .keys()
        .map(|(bridge_id, chain_id)| (*bridge_id as i32, *chain_id as i64))
        .collect_vec();

    let cursors = indexer_checkpoints::Entity::find()
        .filter(
            Expr::tuple([
                Expr::col(indexer_checkpoints::Column::BridgeId).into(),
                Expr::col(indexer_checkpoints::Column::ChainId).into(),
            ])
            .in_tuples(tuples),
        )
        .all(tx)
        .await?
        .into_iter()
        .map(|model| {
            (
                (model.bridge_id as BridgeId, model.chain_id as ChainId),
                Cursor {
                    backward: model.catchup_max_cursor.max(0) as u64,
                    forward: model.realtime_cursor.max(0) as u64,
                },
            )
        })
        .collect();

    Ok(cursors)
}

pub(super) async fn upsert_cursors(
    tx: &DatabaseTransaction,
    cursors: &HashMap<(BridgeId, ChainId), Cursor>,
) -> Result<(), DbErr> {
    let models: Vec<_> = cursors
        .iter()
        .map(
            |((bridge_id, chain_id), cursor)| indexer_checkpoints::ActiveModel {
                bridge_id: ActiveValue::Set(*bridge_id as i32),
                chain_id: ActiveValue::Set(*chain_id as i64),
                catchup_min_cursor: ActiveValue::Set(0),
                catchup_max_cursor: ActiveValue::Set(cursor.backward as i64),
                finality_cursor: ActiveValue::Set(0),
                realtime_cursor: ActiveValue::Set(cursor.forward as i64),
                created_at: ActiveValue::NotSet,
                updated_at: ActiveValue::NotSet,
            },
        )
        .collect();

    indexer_checkpoints::Entity::insert_many(models)
        .on_empty_do_nothing()
        .on_conflict(
            OnConflict::columns([
                indexer_checkpoints::Column::BridgeId,
                indexer_checkpoints::Column::ChainId,
            ])
            .value(
                indexer_checkpoints::Column::CatchupMaxCursor,
                Expr::cust(
                    "LEAST(indexer_checkpoints.catchup_max_cursor, EXCLUDED.catchup_max_cursor)",
                ),
            )
            // Monotonicity insurance, not the healing mechanism: this writer
            // always supplies `0` on insert, and `GREATEST(existing, 0) =
            // existing` can never lower anything, so this rule alone heals
            // nothing. The startup seed (`InterchainDatabase::seed_catchup_floor`)
            // is what heals a stored `0`. The value of this rule is that a
            // future writer supplying a real floor inherits the correct
            // conflict behaviour instead of a plain assignment.
            .value(
                indexer_checkpoints::Column::CatchupMinCursor,
                Expr::cust(
                    "GREATEST(indexer_checkpoints.catchup_min_cursor, EXCLUDED.catchup_min_cursor)",
                ),
            )
            .value(
                indexer_checkpoints::Column::RealtimeCursor,
                Expr::cust(
                    "GREATEST(indexer_checkpoints.realtime_cursor, EXCLUDED.realtime_cursor)",
                ),
            )
            .value(
                indexer_checkpoints::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .to_owned(),
        )
        .exec(tx)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDateTime};
    use interchain_indexer_entity::{
        amb_message_anomalies, bridges, chains, crosschain_messages, crosschain_transfers,
        indexer_checkpoints, pending_messages,
        sea_orm_active_enums::{MessageStatus, TransferAssetLinkage},
        stats_assets,
    };
    use sea_orm::{
        ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait,
        prelude::BigDecimal,
    };

    use super::{
        BridgeId, ConsolidatedMessage, DestinationExecution, Key,
        apply_destination_execution_reconciliation, delete_replaced_messages,
        flush_to_final_storage, reconcile_destination_executions, remove_finalized_from_pending,
    };
    use crate::{InterchainDatabase, test_utils::init_db};

    const BRIDGE_ID: i32 = 7;
    const MESSAGE_ID: i64 = 4242;
    const SRC_CHAIN: i64 = 100;
    const DST_CHAIN: i64 = 200;

    fn ts(secs: i64) -> NaiveDateTime {
        DateTime::from_timestamp(secs, 0).unwrap().naive_utc()
    }

    /// Destination-only finalized row: executed body, no source side. Mirrors
    /// `consolidation::build_destination_only` (src fields NULL, terminal status).
    fn destination_only_completed() -> ConsolidatedMessage {
        ConsolidatedMessage {
            is_final: true,
            replace_existing: false,
            message: crosschain_messages::ActiveModel {
                id: ActiveValue::Set(MESSAGE_ID),
                bridge_id: ActiveValue::Set(BRIDGE_ID),
                status: ActiveValue::Set(MessageStatus::Completed),
                init_timestamp: ActiveValue::Set(ts(2_000)),
                last_update_timestamp: ActiveValue::Set(Some(ts(2_000))),
                src_chain_id: ActiveValue::Set(SRC_CHAIN),
                dst_chain_id: ActiveValue::Set(Some(DST_CHAIN)),
                native_id: ActiveValue::Set(Some(vec![0xAB])),
                src_tx_hash: ActiveValue::Set(None),
                dst_tx_hash: ActiveValue::Set(Some(vec![0xDD])),
                sender_address: ActiveValue::Set(None),
                recipient_address: ActiveValue::Set(Some(vec![0xCC])),
                payload: ActiveValue::Set(None),
                stats_processed: ActiveValue::Set(0),
                protocol_metadata: ActiveValue::Set(None),
                created_at: ActiveValue::NotSet,
                updated_at: ActiveValue::NotSet,
            },
            transfers: vec![],
            amb_confirmations: vec![],
            amb_anomalies: vec![],
        }
    }

    /// Source-only partial row: source seen, destination not yet executed.
    /// Mirrors `consolidation::build_source_led` for `ReadyToClaim` (dst tx NULL,
    /// non-terminal status, source fields populated).
    fn source_only_ready_to_claim() -> ConsolidatedMessage {
        ConsolidatedMessage {
            is_final: false,
            replace_existing: false,
            message: crosschain_messages::ActiveModel {
                id: ActiveValue::Set(MESSAGE_ID),
                bridge_id: ActiveValue::Set(BRIDGE_ID),
                status: ActiveValue::Set(MessageStatus::ReadyToClaim),
                init_timestamp: ActiveValue::Set(ts(1_000)),
                last_update_timestamp: ActiveValue::Set(Some(ts(1_500))),
                src_chain_id: ActiveValue::Set(SRC_CHAIN),
                dst_chain_id: ActiveValue::Set(Some(DST_CHAIN)),
                native_id: ActiveValue::Set(Some(vec![0xAB])),
                src_tx_hash: ActiveValue::Set(Some(vec![0x11])),
                dst_tx_hash: ActiveValue::Set(None),
                sender_address: ActiveValue::Set(Some(vec![0x5E])),
                recipient_address: ActiveValue::Set(Some(vec![0xEE])),
                payload: ActiveValue::Set(Some(vec![0xFA])),
                stats_processed: ActiveValue::Set(0),
                protocol_metadata: ActiveValue::Set(None),
                created_at: ActiveValue::NotSet,
                updated_at: ActiveValue::NotSet,
            },
            transfers: vec![],
            amb_confirmations: vec![],
            amb_anomalies: vec![],
        }
    }

    fn destination_only_completed_with_transfer() -> ConsolidatedMessage {
        let mut entry = destination_only_completed();
        entry.transfers = vec![transfer(
            None,
            Some(990),
            None,
            Some(vec![0xBB]),
            None,
            Some(vec![0x2B]),
        )];
        entry
    }

    fn source_only_ready_to_claim_with_transfer() -> ConsolidatedMessage {
        let mut entry = source_only_ready_to_claim();
        entry.transfers = vec![transfer(
            Some(1_000),
            None,
            Some(vec![0xAA]),
            None,
            Some(vec![0x1A]),
            None,
        )];
        entry
    }

    fn collision_replacement_destination_only_with_transfer() -> ConsolidatedMessage {
        let mut entry = destination_only_completed_with_transfer();
        entry.replace_existing = true;
        entry
    }

    fn amount(value: Option<u64>) -> Option<BigDecimal> {
        value.map(BigDecimal::from)
    }

    fn transfer(
        src_amount: Option<u64>,
        dst_amount: Option<u64>,
        token_src_address: Option<Vec<u8>>,
        token_dst_address: Option<Vec<u8>>,
        sender_address: Option<Vec<u8>>,
        recipient_address: Option<Vec<u8>>,
    ) -> crosschain_transfers::ActiveModel {
        crosschain_transfers::ActiveModel {
            message_id: ActiveValue::Set(MESSAGE_ID),
            bridge_id: ActiveValue::Set(BRIDGE_ID),
            index: ActiveValue::Set(0),
            token_src_chain_id: ActiveValue::Set(SRC_CHAIN),
            token_dst_chain_id: ActiveValue::Set(DST_CHAIN),
            src_amount: ActiveValue::Set(amount(src_amount)),
            dst_amount: ActiveValue::Set(amount(dst_amount)),
            token_src_address: ActiveValue::Set(token_src_address),
            token_dst_address: ActiveValue::Set(token_dst_address),
            sender_address: ActiveValue::Set(sender_address),
            recipient_address: ActiveValue::Set(recipient_address),
            token_ids: ActiveValue::Set(None),
            stats_processed: ActiveValue::Set(0),
            src_stats_asset_id: ActiveValue::Set(None),
            dst_stats_asset_id: ActiveValue::Set(None),
            asset_linkage: ActiveValue::Set(Some(TransferAssetLinkage::Mirror)),
            created_at: ActiveValue::NotSet,
            updated_at: ActiveValue::NotSet,
            id: ActiveValue::NotSet,
        }
    }

    async fn seed_fk_prerequisites(db: &InterchainDatabase) {
        db.upsert_bridges(vec![bridges::ActiveModel {
            id: ActiveValue::Set(BRIDGE_ID),
            name: ActiveValue::Set("test_bridge".to_string()),
            enabled: ActiveValue::Set(true),
            ..Default::default()
        }])
        .await
        .unwrap();
        db.upsert_chains(vec![
            chains::ActiveModel {
                id: ActiveValue::Set(SRC_CHAIN),
                name: ActiveValue::Set("src_chain".to_string()),
                ..Default::default()
            },
            chains::ActiveModel {
                id: ActiveValue::Set(DST_CHAIN),
                name: ActiveValue::Set("dst_chain".to_string()),
                ..Default::default()
            },
        ])
        .await
        .unwrap();
    }

    async fn flush(db: &InterchainDatabase, entry: ConsolidatedMessage) {
        let conn = db.db.as_ref();
        let tx = conn.begin().await.unwrap();
        flush_to_final_storage(&tx, vec![entry]).await.unwrap();
        tx.commit().await.unwrap();
    }

    async fn load(db: &InterchainDatabase) -> crosschain_messages::Model {
        crosschain_messages::Entity::find_by_id((MESSAGE_ID, BRIDGE_ID))
            .one(db.db.as_ref())
            .await
            .unwrap()
            .expect("crosschain_messages row must exist")
    }

    async fn load_transfer(db: &InterchainDatabase) -> crosschain_transfers::Model {
        crosschain_transfers::Entity::find()
            .filter(crosschain_transfers::Column::MessageId.eq(MESSAGE_ID))
            .filter(crosschain_transfers::Column::BridgeId.eq(BRIDGE_ID))
            .filter(crosschain_transfers::Column::Index.eq(0))
            .one(db.db.as_ref())
            .await
            .unwrap()
            .expect("crosschain_transfers row must exist")
    }

    async fn mark_transfer_projected(
        db: &InterchainDatabase,
        stats_processed: i16,
        aid: Option<i64>,
    ) {
        let transfer = load_transfer(db).await;
        let mut active: crosschain_transfers::ActiveModel = transfer.into();
        active.stats_processed = ActiveValue::Set(stats_processed);
        active.src_stats_asset_id = ActiveValue::Set(aid);
        active.dst_stats_asset_id = ActiveValue::Set(aid);
        active.update(db.db.as_ref()).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_late_source_does_not_regress_completed_message() {
        let test_db = init_db("flush_late_source_no_regress").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        // Destination executes and finalizes the row first (the buffer entry is
        // then evicted), then the source side is observed and flushed.
        flush(&db, destination_only_completed()).await;
        flush(&db, source_only_ready_to_claim()).await;

        let row = load(&db).await;
        // Terminal status is not regressed.
        assert_eq!(row.status, MessageStatus::Completed);
        // Destination identity is preserved.
        assert_eq!(row.dst_tx_hash, Some(vec![0xDD]));
        assert_eq!(row.recipient_address, Some(vec![0xCC]));
        // Source side enriches the previously-NULL columns.
        assert_eq!(row.src_tx_hash, Some(vec![0x11]));
        assert_eq!(row.payload, Some(vec![0xFA]));
        assert_eq!(row.sender_address, Some(vec![0x5E]));
        // init_timestamp keeps the earliest (the true source initiation time).
        assert_eq!(row.init_timestamp, ts(1_000));
        // last_update_timestamp keeps the latest known progress.
        assert_eq!(row.last_update_timestamp, Some(ts(2_000)));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_late_destination_completes_source_led_row() {
        let test_db = init_db("flush_late_destination_completes").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        // Reverse order: source-led ReadyToClaim row first, then the destination
        // execution arrives and must drive it to Completed.
        flush(&db, source_only_ready_to_claim()).await;
        flush(&db, destination_only_completed()).await;

        let row = load(&db).await;
        assert_eq!(row.status, MessageStatus::Completed);
        assert_eq!(row.dst_tx_hash, Some(vec![0xDD]));
        assert_eq!(row.recipient_address, Some(vec![0xCC]));
        // Source side recorded earlier is retained.
        assert_eq!(row.src_tx_hash, Some(vec![0x11]));
        assert_eq!(row.payload, Some(vec![0xFA]));
        assert_eq!(row.init_timestamp, ts(1_000));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_late_source_does_not_clear_completed_transfer_destination_side() {
        let test_db = init_db("flush_late_source_no_clear_dst_transfer").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(&db, destination_only_completed_with_transfer()).await;
        flush(&db, source_only_ready_to_claim_with_transfer()).await;

        let transfer = load_transfer(&db).await;
        assert_eq!(transfer.token_src_address, Some(vec![0xAA]));
        assert_eq!(transfer.token_dst_address, Some(vec![0xBB]));
        assert_eq!(transfer.src_amount, Some(BigDecimal::from(1_000)));
        assert_eq!(transfer.dst_amount, Some(BigDecimal::from(990)));
        assert_eq!(transfer.sender_address, Some(vec![0x1A]));
        assert_eq!(transfer.recipient_address, Some(vec![0x2B]));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_late_destination_does_not_clear_source_transfer_side() {
        let test_db = init_db("flush_late_destination_no_clear_src_transfer").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(&db, source_only_ready_to_claim_with_transfer()).await;
        flush(&db, destination_only_completed_with_transfer()).await;

        let transfer = load_transfer(&db).await;
        assert_eq!(transfer.token_src_address, Some(vec![0xAA]));
        assert_eq!(transfer.token_dst_address, Some(vec![0xBB]));
        assert_eq!(transfer.src_amount, Some(BigDecimal::from(1_000)));
        assert_eq!(transfer.dst_amount, Some(BigDecimal::from(990)));
        assert_eq!(transfer.sender_address, Some(vec![0x1A]));
        assert_eq!(transfer.recipient_address, Some(vec![0x2B]));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_late_source_does_not_reset_projected_transfer_stats() {
        let test_db = init_db("flush_late_source_preserves_transfer_stats").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(&db, destination_only_completed_with_transfer()).await;
        let stats_asset_id = stats_assets::Entity::insert(stats_assets::ActiveModel {
            ..Default::default()
        })
        .exec_with_returning(db.db.as_ref())
        .await
        .unwrap()
        .id;
        mark_transfer_projected(&db, 1, Some(stats_asset_id)).await;

        flush(&db, source_only_ready_to_claim_with_transfer()).await;

        let transfer = load_transfer(&db).await;
        assert_eq!(transfer.stats_processed, 1);
        assert_eq!(transfer.src_stats_asset_id, Some(stats_asset_id));
        assert_eq!(transfer.dst_stats_asset_id, Some(stats_asset_id));
        assert_eq!(transfer.token_src_address, Some(vec![0xAA]));
        assert_eq!(transfer.token_dst_address, Some(vec![0xBB]));
    }

    /// Reconstructed-shaped row: unlike `destination_only_completed_with_transfer`
    /// (one nullable side), both `token_src_address` / `token_dst_address` are
    /// already populated — mirroring
    /// `avalanche::consolidation::build_reconstructed_transfer` for an incoming
    /// `SINGLE_HOP_SEND`, whose `sender_address` is explicitly `None` (Decision 4).
    fn reconstructed_incoming_completed_with_transfer() -> ConsolidatedMessage {
        let mut entry = destination_only_completed();
        entry.transfers = vec![transfer(
            Some(1_000),
            Some(1_000),
            Some(vec![0xAA]),
            Some(vec![0xBB]),
            None,
            Some(vec![0x2B]),
        )];
        entry
    }

    /// The `send`-derived row for the same key, arriving later once chain `X`
    /// becomes configured for the bridge and the real `send` event is indexed.
    fn send_derived_completed_with_transfer() -> ConsolidatedMessage {
        let mut entry = destination_only_completed();
        entry.transfers = vec![transfer(
            Some(1_000),
            Some(1_000),
            Some(vec![0xAA]),
            Some(vec![0xBB]),
            Some(vec![0x1A]),
            Some(vec![0x2B]),
        )];
        entry
    }

    /// Regression test for the Avalanche incoming-ICTT-reconstruction merge
    /// contract: a reconstructed row (no `send` observed, built from the ICM
    /// payload) is flushed and stats-projected first; a later `send`-derived
    /// flush for the same `(message_id, bridge_id, index)` must merge into the
    /// same row without double counting and without resetting
    /// `stats_processed` / `stats_asset_id`. `token_src_address` must not
    /// regress — it is byte-identical across both writers by construction
    /// (`TeleporterMessage.originSenderAddress` == the emitting transferrer),
    /// which this asserts rather than assumes.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_reconstructed_incoming_transfer_merges_with_later_send_derived_row() {
        let test_db = init_db("flush_reconstructed_then_send_derived").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(&db, reconstructed_incoming_completed_with_transfer()).await;
        let stats_asset_id = stats_assets::Entity::insert(stats_assets::ActiveModel {
            ..Default::default()
        })
        .exec_with_returning(db.db.as_ref())
        .await
        .unwrap()
        .id;
        mark_transfer_projected(&db, 1, Some(stats_asset_id)).await;

        flush(&db, send_derived_completed_with_transfer()).await;

        let transfer = load_transfer(&db).await;
        assert_eq!(transfer.stats_processed, 1);
        assert_eq!(transfer.src_stats_asset_id, Some(stats_asset_id));
        assert_eq!(transfer.dst_stats_asset_id, Some(stats_asset_id));
        assert_eq!(transfer.token_src_address, Some(vec![0xAA]));
        assert_eq!(transfer.token_dst_address, Some(vec![0xBB]));
        assert_eq!(
            transfer.sender_address,
            Some(vec![0x1A]),
            "the later send-derived flush enriches the previously-NULL sender_address"
        );
    }

    /// `asset_linkage` is write-once via `crosschain_transfers_on_conflict`'s
    /// `COALESCE(stored, incoming)` -- deliberately the reverse argument order
    /// of every other `prefer_incoming` column. `NULL -> value` still applies
    /// (an indexer that only learns the linkage on a later flush can still
    /// state it); `value -> different value` is silently dropped.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_asset_linkage_is_write_once() {
        let test_db = init_db("flush_asset_linkage_write_once").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        // First flush: the indexer has not yet stated the linkage.
        let mut entry = destination_only_completed_with_transfer();
        entry.transfers[0].asset_linkage = ActiveValue::Set(None);
        flush(&db, entry).await;
        assert_eq!(load_transfer(&db).await.asset_linkage, None);

        // Second flush: NULL -> value applies.
        let mut entry = destination_only_completed_with_transfer();
        entry.transfers[0].asset_linkage = ActiveValue::Set(Some(TransferAssetLinkage::Mirror));
        flush(&db, entry).await;
        assert_eq!(
            load_transfer(&db).await.asset_linkage,
            Some(TransferAssetLinkage::Mirror)
        );

        // Third flush: value -> different value is dropped.
        let mut entry = destination_only_completed_with_transfer();
        entry.transfers[0].asset_linkage = ActiveValue::Set(Some(TransferAssetLinkage::Conversion));
        flush(&db, entry).await;
        assert_eq!(
            load_transfer(&db).await.asset_linkage,
            Some(TransferAssetLinkage::Mirror),
            "a stored linkage must never be overwritten by a later, different flush"
        );
    }

    /// Fallback-only completed row: mirrors `SourceData::from_receive` /
    /// `from_execution`'s `Failed` arm after the fix — `sender_address` and
    /// `payload` recovered from the delivered `TeleporterMessage`, plus
    /// `recipient_address` (which the pre-fix code already filled).
    /// `src_tx_hash` stays NULL: no field anywhere carries a transaction hash
    /// for a chain never observed.
    fn fallback_completed_message() -> ConsolidatedMessage {
        ConsolidatedMessage {
            is_final: true,
            replace_existing: false,
            message: crosschain_messages::ActiveModel {
                id: ActiveValue::Set(MESSAGE_ID),
                bridge_id: ActiveValue::Set(BRIDGE_ID),
                status: ActiveValue::Set(MessageStatus::Completed),
                init_timestamp: ActiveValue::Set(ts(2_000)),
                last_update_timestamp: ActiveValue::Set(Some(ts(2_000))),
                src_chain_id: ActiveValue::Set(SRC_CHAIN),
                dst_chain_id: ActiveValue::Set(Some(DST_CHAIN)),
                native_id: ActiveValue::Set(Some(vec![0xAB])),
                src_tx_hash: ActiveValue::Set(None),
                dst_tx_hash: ActiveValue::Set(Some(vec![0xDD])),
                sender_address: ActiveValue::Set(Some(vec![0x5E])),
                recipient_address: ActiveValue::Set(Some(vec![0xEE])),
                payload: ActiveValue::Set(Some(vec![0xFA])),
                stats_processed: ActiveValue::Set(0),
                protocol_metadata: ActiveValue::Set(None),
                created_at: ActiveValue::NotSet,
                updated_at: ActiveValue::NotSet,
            },
            transfers: vec![],
            amb_confirmations: vec![],
            amb_anomalies: vec![],
        }
    }

    /// The `send`-derived row for the same key, arriving later once chain `X`
    /// becomes configured for the bridge and the real `send` event is indexed.
    /// `sender_address` / `recipient_address` / `payload` are byte-identical to
    /// the fallback values above — same underlying `TeleporterMessage`,
    /// observed twice. `src_tx_hash` is the one field the fallback path could
    /// never fill, now known.
    fn send_derived_message_after_fallback() -> ConsolidatedMessage {
        let mut entry = fallback_completed_message();
        entry.message.src_tx_hash = ActiveValue::Set(Some(vec![0x11]));
        entry
    }

    /// DB-backed round trip for the fallback-fields fix: a fallback-only
    /// message (`sender_address`/`payload` recovered per-commit, `src_tx_hash`
    /// NULL) is flushed to terminal status, then a `send`-driven flush for the
    /// same `(id, bridge_id)` arrives once chain `X` is configured. The merge
    /// must be a same-value no-op for `sender_address`/`payload` — proving the
    /// "harmless once the real send arrives" claim instead of assuming it —
    /// while `src_tx_hash` is newly enriched.
    ///
    /// `recipient_address` is the odd one out: `crosschain_messages_on_conflict`
    /// uses `keep_existing_if_terminal` for it (unlike the `COALESCE`
    /// `prefer_incoming` policy for `sender_address`/`payload`/`src_tx_hash`),
    /// so once `status` is terminal it is locked to the stored value forever —
    /// even a *different* incoming value would be silently discarded here, not
    /// just this identical one. A recipient left NULL on first write can never
    /// be patched by a later flush.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_send_derived_merge_is_no_op_for_already_recovered_fallback_fields() {
        let test_db = init_db("flush_fallback_then_send_derived_message_fields").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(&db, fallback_completed_message()).await;
        flush(&db, send_derived_message_after_fallback()).await;

        let row = load(&db).await;
        assert_eq!(row.status, MessageStatus::Completed);
        assert_eq!(
            row.sender_address,
            Some(vec![0x5E]),
            "already-correct sender_address must be unchanged by the later merge"
        );
        assert_eq!(
            row.payload,
            Some(vec![0xFA]),
            "already-correct payload must be unchanged by the later merge"
        );
        assert_eq!(
            row.src_tx_hash,
            Some(vec![0x11]),
            "src_tx_hash is newly known now that chain X is configured"
        );
        assert_eq!(
            row.recipient_address,
            Some(vec![0xEE]),
            "recipient_address stays locked to the fallback value once terminal"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_collision_replacement_deletes_stale_source_message_and_transfer() {
        let test_db = init_db("flush_collision_replace_deletes_stale_source").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(&db, source_only_ready_to_claim_with_transfer()).await;
        flush(&db, collision_replacement_destination_only_with_transfer()).await;

        let row = load(&db).await;
        assert_eq!(row.status, MessageStatus::Completed);
        assert_eq!(row.src_tx_hash, None);
        assert_eq!(row.payload, None);
        assert_eq!(row.sender_address, None);
        assert_eq!(row.dst_tx_hash, Some(vec![0xDD]));
        assert_eq!(row.recipient_address, Some(vec![0xCC]));
        assert_eq!(row.init_timestamp, ts(2_000));

        let transfer = load_transfer(&db).await;
        assert_eq!(transfer.token_src_address, None);
        assert_eq!(transfer.src_amount, None);
        assert_eq!(transfer.sender_address, None);
        assert_eq!(transfer.token_dst_address, Some(vec![0xBB]));
        assert_eq!(transfer.dst_amount, Some(BigDecimal::from(990)));
        assert_eq!(transfer.recipient_address, Some(vec![0x2B]));
    }

    // --- `upsert_cursors`' `catchup_min_cursor` GREATEST insurance.
    // This rule alone heals nothing —
    // `upsert_cursors` always supplies `0` on insert — it only guarantees a
    // once-seeded floor can never be lowered by a later cursor-maintenance
    // write. See `InterchainDatabase::seed_catchup_floor` for the healing
    // mechanism itself. ---

    async fn upsert_cursors_once(db: &InterchainDatabase, cursor: super::Cursor) {
        let conn = db.db.as_ref();
        let tx = conn.begin().await.unwrap();
        let cursors = std::collections::HashMap::from([(
            (BRIDGE_ID as super::BridgeId, SRC_CHAIN as super::ChainId),
            cursor,
        )]);
        super::upsert_cursors(&tx, &cursors).await.unwrap();
        tx.commit().await.unwrap();
    }

    async fn load_checkpoint(db: &InterchainDatabase) -> indexer_checkpoints::Model {
        db.get_checkpoint(BRIDGE_ID as u64, SRC_CHAIN as u64)
            .await
            .unwrap()
            .expect("checkpoint row must exist")
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_upsert_cursors_zero_min_cursor_is_noop_after_seed_via_own_insert_then_conflict() {
        let test_db = init_db("upsert_cursors_noop_insert_then_conflict").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        // `upsert_cursors`' own INSERT branch: no row exists yet, so this
        // creates one with `catchup_min_cursor = 0`.
        upsert_cursors_once(
            &db,
            super::Cursor {
                backward: 5_000,
                forward: 5_000,
            },
        )
        .await;
        assert_eq!(load_checkpoint(&db).await.catchup_min_cursor, 0);

        // A different writer (the startup seed) raises the floor via its own
        // `GREATEST` conflict rule.
        db.seed_catchup_floor(BRIDGE_ID, SRC_CHAIN, 1_000, 5_000, 5_000)
            .await
            .unwrap();
        assert_eq!(load_checkpoint(&db).await.catchup_min_cursor, 1_000);

        // `upsert_cursors` runs again for the same pair: this time the row
        // exists, so it goes through its own ON CONFLICT branch, still
        // supplying `catchup_min_cursor = 0`. `GREATEST(1000, 0) = 1000`
        // must leave the floor untouched.
        upsert_cursors_once(
            &db,
            super::Cursor {
                backward: 4_500,
                forward: 5_500,
            },
        )
        .await;

        let checkpoint = load_checkpoint(&db).await;
        assert_eq!(checkpoint.catchup_min_cursor, 1_000);
        assert_eq!(checkpoint.catchup_max_cursor, 4_500);
        assert_eq!(checkpoint.realtime_cursor, 5_500);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_upsert_cursors_zero_min_cursor_is_noop_after_seed_via_conflict_only() {
        let test_db = init_db("upsert_cursors_noop_conflict_only").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        // The row is created entirely by the seed; `upsert_cursors` never
        // exercises its own INSERT branch for this pair.
        db.seed_catchup_floor(BRIDGE_ID, SRC_CHAIN, 1_000, 5_000, 5_000)
            .await
            .unwrap();
        assert_eq!(load_checkpoint(&db).await.catchup_min_cursor, 1_000);

        upsert_cursors_once(
            &db,
            super::Cursor {
                backward: 4_500,
                forward: 5_500,
            },
        )
        .await;

        let checkpoint = load_checkpoint(&db).await;
        assert_eq!(checkpoint.catchup_min_cursor, 1_000);
        assert_eq!(checkpoint.catchup_max_cursor, 4_500);
        assert_eq!(checkpoint.realtime_cursor, 5_500);
    }

    // --- protocol_metadata merge at upsert (avalanche-unresolved-destinations) ---

    fn unresolved_meta_json(tag: &str) -> serde_json::Value {
        serde_json::json!({
            "unresolved_destination": {
                "reason": "Unable to resolve the destination chain",
                "protocol": "avalanche_icm",
                "blockchain_id": format!("0x{tag}"),
                "blockchain_id_cb58": tag,
                "network": "mainnet",
            }
        })
    }

    /// A `multiple_executions` namespace value shaped exactly like
    /// [`reconcile_destination_executions`] would assemble it, for exercising
    /// the `||`-merge rule directly through `merge_test_row` without going
    /// through the reconciliation machinery.
    fn multiple_executions_meta_json(tag: u8) -> serde_json::Value {
        serde_json::json!({
            "multiple_executions": {
                "protocol": "xdai",
                "additional_executions": [
                    {
                        "transaction_hash": alloy::hex::encode_prefixed([tag]),
                        "timestamp": "2025-07-17T20:52:50.000Z",
                    }
                ]
            }
        })
    }

    /// A row shaped like a non-terminal, possibly-unresolved-destination
    /// send: `dst_chain_id` / `protocol_metadata` are the two values under
    /// test, everything else fixed so only the merge rule under test varies.
    fn merge_test_row(
        dst_chain_id: Option<i64>,
        protocol_metadata: Option<serde_json::Value>,
    ) -> ConsolidatedMessage {
        ConsolidatedMessage {
            is_final: false,
            replace_existing: false,
            message: crosschain_messages::ActiveModel {
                id: ActiveValue::Set(MESSAGE_ID),
                bridge_id: ActiveValue::Set(BRIDGE_ID),
                status: ActiveValue::Set(MessageStatus::Initiated),
                init_timestamp: ActiveValue::Set(ts(1_000)),
                last_update_timestamp: ActiveValue::Set(Some(ts(1_000))),
                src_chain_id: ActiveValue::Set(SRC_CHAIN),
                dst_chain_id: ActiveValue::Set(dst_chain_id),
                native_id: ActiveValue::Set(Some(vec![0xAB])),
                src_tx_hash: ActiveValue::Set(Some(vec![0x11])),
                dst_tx_hash: ActiveValue::Set(None),
                sender_address: ActiveValue::Set(Some(vec![0x5E])),
                recipient_address: ActiveValue::Set(Some(vec![0xEE])),
                payload: ActiveValue::Set(Some(vec![0xFA])),
                stats_processed: ActiveValue::Set(0),
                protocol_metadata: ActiveValue::Set(protocol_metadata),
                created_at: ActiveValue::NotSet,
                updated_at: ActiveValue::NotSet,
            },
            transfers: vec![],
            amb_confirmations: vec![],
            amb_anomalies: vec![],
        }
    }

    /// stored dst NULL + meta present, incoming dst NULL + meta present
    /// (fresh) -> result is the incoming metadata: an ordinary diagnostics
    /// refresh while the destination is still unresolved.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_protocol_metadata_merge_both_unresolved_incoming_wins() {
        let test_db = init_db("protocol_metadata_merge_both_unresolved_incoming_wins").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(&db, merge_test_row(None, Some(unresolved_meta_json("aa")))).await;
        flush(&db, merge_test_row(None, Some(unresolved_meta_json("bb")))).await;

        let row = load(&db).await;
        assert_eq!(row.dst_chain_id, None);
        assert_eq!(row.protocol_metadata, Some(unresolved_meta_json("bb")));
    }

    /// stored dst NULL + meta present, incoming dst NULL + meta NULL ->
    /// result is the stored metadata: a flush that carries no new
    /// diagnostics must not erase the diagnostics already on record.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_protocol_metadata_merge_incoming_null_keeps_stored() {
        let test_db = init_db("protocol_metadata_merge_incoming_null_keeps_stored").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(&db, merge_test_row(None, Some(unresolved_meta_json("aa")))).await;
        flush(&db, merge_test_row(None, None)).await;

        let row = load(&db).await;
        assert_eq!(row.dst_chain_id, None);
        assert_eq!(row.protocol_metadata, Some(unresolved_meta_json("aa")));
    }

    /// stored dst known + `multiple_executions` present, incoming dst known +
    /// meta NULL -> result is the stored `multiple_executions`, preserved.
    ///
    /// Sibling of `test_protocol_metadata_merge_incoming_null_keeps_stored`
    /// for a different namespace, deliberately: that test's stored value is
    /// `unresolved_destination`, which has its own deletion branch in
    /// `crosschain_messages_on_conflict` (`- 'unresolved_destination'`, fired
    /// once the merged `dst_chain_id` is known) -- so it cannot pin the
    /// general `||`-merge stickiness rule that every *other* namespace relies
    /// on. `multiple_executions` has no such branch, and both flushes here
    /// keep `dst_chain_id` known throughout, so this is the case a late
    /// source-only flush (protocol_metadata = NULL incoming, e.g. a
    /// re-processed or duplicate source-side observation) actually hits: it
    /// must not drop an already-recorded late-execution array.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_protocol_metadata_merge_multiple_executions_incoming_null_keeps_stored() {
        let test_db =
            init_db("protocol_metadata_merge_multiple_executions_incoming_null_keeps_stored").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(
            &db,
            merge_test_row(Some(DST_CHAIN), Some(multiple_executions_meta_json(0xFE))),
        )
        .await;
        flush(&db, merge_test_row(Some(DST_CHAIN), None)).await;

        let row = load(&db).await;
        assert_eq!(row.dst_chain_id, Some(DST_CHAIN));
        assert_eq!(
            row.protocol_metadata,
            Some(multiple_executions_meta_json(0xFE)),
            "a late source-only flush with no metadata of its own must not \
             drop the already-recorded multiple_executions array"
        );
    }

    /// stored dst NULL + meta present, incoming dst known + meta NULL ->
    /// destination resolved: metadata must be cleared to NULL, not merged.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_protocol_metadata_merge_resolution_clears_metadata() {
        let test_db = init_db("protocol_metadata_merge_resolution_clears_metadata").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(&db, merge_test_row(None, Some(unresolved_meta_json("aa")))).await;
        flush(&db, merge_test_row(Some(DST_CHAIN), None)).await;

        let row = load(&db).await;
        assert_eq!(row.dst_chain_id, Some(DST_CHAIN));
        assert_eq!(
            row.protocol_metadata, None,
            "destination is now known: a resolved flush must clear metadata"
        );
    }

    /// stored dst known + meta NULL, incoming dst NULL + meta present
    /// (stale) -> a stale unresolved observation must neither resurrect old
    /// diagnostics nor null out the already-known destination.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_protocol_metadata_merge_stale_unresolved_does_not_regress() {
        let test_db = init_db("protocol_metadata_merge_stale_unresolved_does_not_regress").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(&db, merge_test_row(Some(DST_CHAIN), None)).await;
        flush(&db, merge_test_row(None, Some(unresolved_meta_json("aa")))).await;

        let row = load(&db).await;
        assert_eq!(
            row.dst_chain_id,
            Some(DST_CHAIN),
            "a stale NULL destination must not overwrite the already-known one"
        );
        assert_eq!(
            row.protocol_metadata, None,
            "a stale unresolved snapshot must not resurrect once destination is known"
        );
    }

    /// stored dst NULL + meta NULL, incoming dst NULL + meta NULL -> result
    /// stays NULL: an empty JSON object must never reach the column.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_protocol_metadata_merge_all_null_stays_null() {
        let test_db = init_db("protocol_metadata_merge_all_null_stays_null").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(&db, merge_test_row(None, None)).await;
        flush(&db, merge_test_row(None, None)).await;

        let row = load(&db).await;
        assert_eq!(row.dst_chain_id, None);
        assert_eq!(row.protocol_metadata, None);
    }

    /// Cohort size that exceeds PostgreSQL's planner stack for a row-valued
    /// `IN` (measured threshold: 7 500-8 000 tuples at the default
    /// `max_stack_depth = 2048kB`), while staying far below the bind-parameter
    /// ceiling that the old `run_in_batches(&keys, 2, …)` sizing respected.
    ///
    /// That combination is the point: these statements were *bind-safe* and
    /// still crashed. Under the old sizing the whole cohort went out as one
    /// 10 000-tuple statement and Postgres answered `stack depth limit
    /// exceeded`; `ROW_IN_KEY_CHUNK` splits it into five statements instead.
    const OVER_STACK_DEPTH_COHORT: i64 = 10_000;

    /// Regression test for the statement that crashed buffer maintenance 72
    /// times during the 2026-09-16 verification run. No rows are seeded — the
    /// failure is in parsing/planning, so an empty table reproduces it.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_remove_finalized_from_pending_cohort_over_stack_depth() {
        let test_db = init_db("remove_finalized_pending_over_stack_depth").await;
        let db = InterchainDatabase::new(test_db.client());

        let keys: Vec<Key> = (1..=OVER_STACK_DEPTH_COHORT)
            .map(|id| Key::new(id, BRIDGE_ID as BridgeId))
            .collect();

        let conn = db.db.as_ref();
        let tx = conn.begin().await.unwrap();
        remove_finalized_from_pending(&tx, &keys).await.unwrap();
        tx.commit().await.unwrap();
    }

    /// Sibling of the above: same row-valued `IN` DELETE shape, same cohort,
    /// different table. It never fired in production only because replacement
    /// cohorts stayed small — it carried the identical exposure.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_delete_replaced_messages_cohort_over_stack_depth() {
        let test_db = init_db("delete_replaced_messages_over_stack_depth").await;
        let db = InterchainDatabase::new(test_db.client());

        let pks: Vec<(i64, i32)> = (1..=OVER_STACK_DEPTH_COHORT)
            .map(|id| (id, BRIDGE_ID))
            .collect();

        let conn = db.db.as_ref();
        let tx = conn.begin().await.unwrap();
        delete_replaced_messages(&tx, &pks).await.unwrap();
        tx.commit().await.unwrap();
    }

    // --- `reconcile_destination_executions` / `apply_destination_execution_reconciliation` ---
    // (xDai multiple-execution anomalies)

    fn destination_execution(
        tx_hash: Vec<u8>,
        native_id: Vec<u8>,
        log_index: Option<i64>,
        block_number: i64,
        block_timestamp: NaiveDateTime,
        detail: &str,
    ) -> DestinationExecution {
        DestinationExecution {
            key: Key::new(MESSAGE_ID, BRIDGE_ID as BridgeId),
            native_id,
            chain_id: DST_CHAIN,
            tx_hash,
            log_index,
            block_number,
            block_timestamp,
            executor: Some(vec![0xEE]),
            src_chain_id: Some(SRC_CHAIN),
            dst_chain_id: Some(DST_CHAIN),
            detail: detail.to_string(),
        }
    }

    fn destination_only_completed_with_transfer_and_tx(
        tx_hash: Vec<u8>,
        dst_amount: u64,
    ) -> ConsolidatedMessage {
        let mut entry = destination_only_completed();
        entry.message.dst_tx_hash = ActiveValue::Set(Some(tx_hash));
        entry.transfers = vec![transfer(
            None,
            Some(dst_amount),
            None,
            Some(vec![0xBB]),
            None,
            Some(vec![0x2B]),
        )];
        entry
    }

    async fn count_destination_execution_anomalies(db: &InterchainDatabase) -> usize {
        amb_message_anomalies::Entity::find()
            .filter(amb_message_anomalies::Column::BridgeId.eq(BRIDGE_ID))
            .filter(amb_message_anomalies::Column::BufferKey.eq(MESSAGE_ID))
            .filter(amb_message_anomalies::Column::EventKind.eq("destination_execution"))
            .all(db.db.as_ref())
            .await
            .unwrap()
            .len()
    }

    async fn additional_executions_of(db: &InterchainDatabase) -> Vec<String> {
        let row = load(db).await;
        crate::protocol_metadata::ProtocolMetadata::from_json_value(row.protocol_metadata)
            .and_then(|metadata| metadata.multiple_executions)
            .map(|multiple| {
                multiple
                    .additional_executions
                    .into_iter()
                    .map(|execution| execution.transaction_hash)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Two executions of one canonical key observed in the same maintenance
    /// cycle, neither previously stored: the first (matching the
    /// `ConsolidatedMessage` this cycle also writes) stays canonical, the
    /// second becomes exactly one anomaly row and one `multiple_executions`
    /// entry.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_reconcile_two_executions_coexist_before_flush_first_wins() {
        let test_db = init_db("reconcile_two_executions_coexist_first_wins").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        let key = Key::new(MESSAGE_ID, BRIDGE_ID as BridgeId);
        let mut entries = vec![destination_only_completed_with_transfer()];
        let observations = vec![(
            key,
            vec![
                destination_execution(vec![0xDD], vec![0xAA; 32], Some(1), 20, ts(2_000), "first"),
                destination_execution(vec![0xFE], vec![0xBB; 32], Some(2), 25, ts(2_500), "second"),
            ],
        )];

        let conn = db.db.as_ref();
        let tx = conn.begin().await.unwrap();
        let reconciliation = reconcile_destination_executions(&tx, &mut entries, &observations)
            .await
            .unwrap();
        assert_eq!(reconciliation.resolved_keys, vec![key]);
        flush_to_final_storage(&tx, entries).await.unwrap();
        apply_destination_execution_reconciliation(&tx, &reconciliation)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let row = load(&db).await;
        assert_eq!(
            row.dst_tx_hash,
            Some(vec![0xDD]),
            "the first-seen execution stays canonical"
        );
        assert_eq!(count_destination_execution_anomalies(&db).await, 1);
        assert_eq!(
            additional_executions_of(&db).await,
            vec![alloy::hex::encode_prefixed([0xFEu8])]
        );
    }

    /// A late alias for an already-finalized (and, implicitly, evicted)
    /// canonical row: the stored execution wins even though this cycle's
    /// fresh `ConsolidatedMessage` (built by a buffer instance that never saw
    /// the original canonical execution) disagrees, and the mismatched
    /// message/transfer fields are neutralized before the upsert.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_reconcile_late_alias_after_finalization_stored_wins_no_overwrite() {
        let test_db = init_db("reconcile_late_alias_stored_wins").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        // Step 1: the canonical execution (0xDD) is already stored (and, in a
        // real run, evicted from hot afterward).
        flush(&db, destination_only_completed_with_transfer()).await;

        // Step 2: a fresh buffer instance sees a different execution (0xFE)
        // first and builds a `ConsolidatedMessage` around it -- exactly the
        // "buffer lost memory of the stored canonical" scenario.
        let key = Key::new(MESSAGE_ID, BRIDGE_ID as BridgeId);
        let mut entries = vec![destination_only_completed_with_transfer_and_tx(
            vec![0xFE],
            995,
        )];
        let observations = vec![(
            key,
            vec![destination_execution(
                vec![0xFE],
                vec![0xBB; 32],
                Some(3),
                30,
                ts(3_000),
                "late",
            )],
        )];

        let conn = db.db.as_ref();
        let tx = conn.begin().await.unwrap();
        let reconciliation = reconcile_destination_executions(&tx, &mut entries, &observations)
            .await
            .unwrap();
        flush_to_final_storage(&tx, entries).await.unwrap();
        apply_destination_execution_reconciliation(&tx, &reconciliation)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let row = load(&db).await;
        assert_eq!(
            row.dst_tx_hash,
            Some(vec![0xDD]),
            "dst_tx_hash must not regress"
        );
        assert_eq!(
            row.recipient_address,
            Some(vec![0xCC]),
            "recipient_address must not regress"
        );
        assert_eq!(row.status, MessageStatus::Completed);
        assert_eq!(
            row.last_update_timestamp,
            Some(ts(2_000)),
            "last_update_timestamp must stay the canonical execution's, not the late one's"
        );
        let transfer_row = load_transfer(&db).await;
        assert_eq!(
            transfer_row.dst_amount,
            Some(BigDecimal::from(990)),
            "dst_amount must stay the canonical execution's, not the late one's"
        );

        assert_eq!(count_destination_execution_anomalies(&db).await, 1);
        assert_eq!(
            additional_executions_of(&db).await,
            vec![alloy::hex::encode_prefixed([0xFEu8])]
        );
    }

    /// Both sides of a raw-hash xDai message whose source receipt carries no
    /// recognized source event: `resolve_input`'s `(None, None)` arm takes
    /// the recipient *and* `src_amount` from the completion itself, so two
    /// distinct executions of the same canonical key disagree on every
    /// completion-derived column -- message and transfer alike.
    fn raw_hash_no_source_event_completed(
        dst_tx_hash: Vec<u8>,
        recipient: Vec<u8>,
        completion_value: u64,
        completed_at: NaiveDateTime,
    ) -> ConsolidatedMessage {
        let mut entry = destination_only_completed();
        entry.message.src_tx_hash = ActiveValue::Set(Some(vec![0x11]));
        entry.message.sender_address = ActiveValue::Set(Some(vec![0x5E]));
        entry.message.dst_tx_hash = ActiveValue::Set(Some(dst_tx_hash));
        entry.message.recipient_address = ActiveValue::Set(Some(recipient.clone()));
        entry.message.last_update_timestamp = ActiveValue::Set(Some(completed_at));
        entry.transfers = vec![transfer(
            Some(completion_value),
            Some(completion_value),
            Some(vec![0xAA]),
            Some(vec![0xBB]),
            Some(vec![0x5E]),
            Some(recipient),
        )];
        entry
    }

    /// A late, distinct execution of an already-finalized (and evicted)
    /// raw-hash/no-source-event message must not rewrite **any** column of
    /// the stored canonical transfer. Unlike the destination-only fixture in
    /// `test_reconcile_late_alias_after_finalization_stored_wins_no_overwrite`,
    /// the late transfer here carries a different `recipient_address` and
    /// `src_amount`, which the `COALESCE`-based transfer upsert would
    /// otherwise take from the late completion. The stored projection state
    /// must also survive untouched, so the already-counted row is neither
    /// re-counted nor left disagreeing with the amount it was counted at.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_reconcile_late_raw_hash_execution_stored_wins_keeps_canonical_transfer() {
        let test_db = init_db("reconcile_late_raw_hash_stored_wins_keeps_transfer").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        // Step 1: the canonical execution (0xDD, recipient 0x2B, 990) is
        // stored and stats-projected.
        flush(
            &db,
            raw_hash_no_source_event_completed(vec![0xDD], vec![0x2B], 990, ts(2_000)),
        )
        .await;
        let stats_asset_id = stats_assets::Entity::insert(stats_assets::ActiveModel {
            ..Default::default()
        })
        .exec_with_returning(db.db.as_ref())
        .await
        .unwrap()
        .id;
        mark_transfer_projected(&db, 1, Some(stats_asset_id)).await;
        let canonical_transfer = load_transfer(&db).await;

        // Step 2: a fresh buffer instance sees only a later, distinct
        // execution (0xFE, recipient 0x3C, 995) and consolidates around it.
        let key = Key::new(MESSAGE_ID, BRIDGE_ID as BridgeId);
        let mut entries = vec![raw_hash_no_source_event_completed(
            vec![0xFE],
            vec![0x3C],
            995,
            ts(3_000),
        )];
        let observations = vec![(
            key,
            vec![destination_execution(
                vec![0xFE],
                vec![0x11; 32],
                Some(3),
                30,
                ts(3_000),
                "late",
            )],
        )];

        let conn = db.db.as_ref();
        let tx = conn.begin().await.unwrap();
        let reconciliation = reconcile_destination_executions(&tx, &mut entries, &observations)
            .await
            .unwrap();
        assert_eq!(reconciliation.resolved_keys, vec![key]);
        flush_to_final_storage(&tx, entries).await.unwrap();
        apply_destination_execution_reconciliation(&tx, &reconciliation)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let row = load(&db).await;
        assert_eq!(row.status, MessageStatus::Completed);
        assert_eq!(
            row.dst_tx_hash,
            Some(vec![0xDD]),
            "dst_tx_hash must stay the canonical execution's"
        );
        assert_eq!(
            row.recipient_address,
            Some(vec![0x2B]),
            "message recipient_address must stay the canonical execution's"
        );
        assert_eq!(
            row.last_update_timestamp,
            Some(ts(2_000)),
            "last_update_timestamp must stay the canonical execution's"
        );

        let transfer_row = load_transfer(&db).await;
        assert_eq!(
            transfer_row.recipient_address,
            Some(vec![0x2B]),
            "transfer recipient_address must stay the canonical execution's"
        );
        assert_eq!(
            transfer_row.src_amount,
            Some(BigDecimal::from(990)),
            "src_amount must stay the canonical execution's"
        );
        assert_eq!(
            transfer_row.dst_amount,
            Some(BigDecimal::from(990)),
            "dst_amount must stay the canonical execution's"
        );
        assert_eq!(
            transfer_row.stats_processed, 1,
            "the already-counted transfer must not be reset for re-projection"
        );
        assert_eq!(transfer_row.src_stats_asset_id, Some(stats_asset_id));
        assert_eq!(transfer_row.dst_stats_asset_id, Some(stats_asset_id));
        assert_eq!(
            transfer_row, canonical_transfer,
            "no column of the stored canonical transfer may change"
        );

        assert_eq!(count_destination_execution_anomalies(&db).await, 1);
        assert_eq!(
            additional_executions_of(&db).await,
            vec![alloy::hex::encode_prefixed([0xFEu8])]
        );
    }

    /// A *second*, distinct late execution noticed after the row already
    /// carries one `multiple_executions` entry must add exactly one more
    /// entry and anomaly row, preserving the one already recorded --
    /// `reconcile_destination_executions` reads the stored array before
    /// appending, never replaces it wholesale.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_reconcile_second_late_execution_adds_one_entry_and_keeps_the_first() {
        let test_db = init_db("reconcile_second_late_execution_adds_one_entry").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(&db, destination_only_completed_with_transfer()).await;
        let key = Key::new(MESSAGE_ID, BRIDGE_ID as BridgeId);

        // First late execution (0xFE), reconciled and committed on its own.
        {
            let mut entries: Vec<ConsolidatedMessage> = Vec::new();
            let observations = vec![(
                key,
                vec![destination_execution(
                    vec![0xFE],
                    vec![0xBB; 32],
                    Some(3),
                    30,
                    ts(3_000),
                    "late",
                )],
            )];
            let conn = db.db.as_ref();
            let tx = conn.begin().await.unwrap();
            let reconciliation = reconcile_destination_executions(&tx, &mut entries, &observations)
                .await
                .unwrap();
            flush_to_final_storage(&tx, entries).await.unwrap();
            apply_destination_execution_reconciliation(&tx, &reconciliation)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }
        assert_eq!(count_destination_execution_anomalies(&db).await, 1);
        assert_eq!(
            additional_executions_of(&db).await,
            vec![alloy::hex::encode_prefixed([0xFEu8])]
        );

        // A second, distinct late execution (0xFC), reconciled separately.
        {
            let mut entries: Vec<ConsolidatedMessage> = Vec::new();
            let observations = vec![(
                key,
                vec![destination_execution(
                    vec![0xFC],
                    vec![0xCC; 32],
                    Some(4),
                    40,
                    ts(4_000),
                    "late",
                )],
            )];
            let conn = db.db.as_ref();
            let tx = conn.begin().await.unwrap();
            let reconciliation = reconcile_destination_executions(&tx, &mut entries, &observations)
                .await
                .unwrap();
            flush_to_final_storage(&tx, entries).await.unwrap();
            apply_destination_execution_reconciliation(&tx, &reconciliation)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }

        assert_eq!(
            count_destination_execution_anomalies(&db).await,
            2,
            "both late executions must have their own anomaly row"
        );
        let mut executions = additional_executions_of(&db).await;
        executions.sort();
        let mut expected = vec![
            alloy::hex::encode_prefixed([0xFEu8]),
            alloy::hex::encode_prefixed([0xFCu8]),
        ];
        expected.sort();
        assert_eq!(
            executions, expected,
            "the array must contain both executions, the first one preserved"
        );
    }

    /// Reprocessing the same late execution (e.g. a re-scanned or replayed
    /// log bumping the buffer item's version again) must not grow either the
    /// anomaly table or the `multiple_executions` array a second time.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_reconcile_repeated_processing_of_late_execution_stays_one_row() {
        let test_db = init_db("reconcile_repeated_late_execution_stays_one_row").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(&db, destination_only_completed_with_transfer()).await;

        let key = Key::new(MESSAGE_ID, BRIDGE_ID as BridgeId);
        let observations = vec![(
            key,
            vec![destination_execution(
                vec![0xFE],
                vec![0xBB; 32],
                Some(3),
                30,
                ts(3_000),
                "late",
            )],
        )];

        for _ in 0..2 {
            let mut entries: Vec<ConsolidatedMessage> = Vec::new();
            let conn = db.db.as_ref();
            let tx = conn.begin().await.unwrap();
            let reconciliation = reconcile_destination_executions(&tx, &mut entries, &observations)
                .await
                .unwrap();
            flush_to_final_storage(&tx, entries).await.unwrap();
            apply_destination_execution_reconciliation(&tx, &reconciliation)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }

        assert_eq!(
            count_destination_execution_anomalies(&db).await,
            1,
            "reprocessing the same late execution must not duplicate the anomaly row"
        );
        assert_eq!(
            additional_executions_of(&db).await,
            vec![alloy::hex::encode_prefixed([0xFEu8])],
            "reprocessing must not duplicate the array entry either"
        );
    }

    /// An observation whose `tx_hash` matches the canonical one exactly (an
    /// idempotent replay of the canonical execution) must never become an
    /// anomaly, and must never touch `protocol_metadata`.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_reconcile_exact_replay_of_canonical_is_not_an_anomaly() {
        let test_db = init_db("reconcile_exact_replay_of_canonical_is_not_an_anomaly").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        flush(&db, destination_only_completed_with_transfer()).await;

        let key = Key::new(MESSAGE_ID, BRIDGE_ID as BridgeId);
        let observations = vec![(
            key,
            vec![destination_execution(
                vec![0xDD],
                vec![0xAA; 32],
                Some(1),
                20,
                ts(2_000),
                "replay",
            )],
        )];

        let mut entries: Vec<ConsolidatedMessage> = Vec::new();
        let conn = db.db.as_ref();
        let tx = conn.begin().await.unwrap();
        let reconciliation = reconcile_destination_executions(&tx, &mut entries, &observations)
            .await
            .unwrap();
        flush_to_final_storage(&tx, entries).await.unwrap();
        apply_destination_execution_reconciliation(&tx, &reconciliation)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(count_destination_execution_anomalies(&db).await, 0);
        assert_eq!(load(&db).await.protocol_metadata, None);
    }

    /// A destination-only observation with neither a stored canonical row nor
    /// a `ConsolidatedMessage` this cycle (the source has not arrived, and
    /// this is the very first time the destination side is seen) must not
    /// write anything: not an anomaly row, not a metadata patch, and the key
    /// must not be reported resolved.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_reconcile_destination_only_without_anything_to_write_against_stays_unresolved() {
        let test_db = init_db("reconcile_destination_only_without_anchor_stays_unresolved").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        let key = Key::new(MESSAGE_ID, BRIDGE_ID as BridgeId);
        let observations = vec![(
            key,
            vec![destination_execution(
                vec![0xDD],
                vec![0xAA; 32],
                Some(1),
                20,
                ts(2_000),
                "unanchored",
            )],
        )];

        let mut entries: Vec<ConsolidatedMessage> = Vec::new();
        let conn = db.db.as_ref();
        let tx = conn.begin().await.unwrap();
        let reconciliation = reconcile_destination_executions(&tx, &mut entries, &observations)
            .await
            .unwrap();
        assert!(
            reconciliation.resolved_keys.is_empty(),
            "nothing to write against yet: the key must not be reported resolved"
        );
        flush_to_final_storage(&tx, entries).await.unwrap();
        apply_destination_execution_reconciliation(&tx, &reconciliation)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(count_destination_execution_anomalies(&db).await, 0);
        assert!(
            crosschain_messages::Entity::find_by_id((MESSAGE_ID, BRIDGE_ID))
                .one(db.db.as_ref())
                .await
                .unwrap()
                .is_none(),
            "no phantom row must appear"
        );
    }

    /// The whole reconciliation (anomaly row, metadata patch, and the
    /// upserted message/transfer) rolls back together when the transaction is
    /// never committed.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_reconcile_and_apply_roll_back_together() {
        let test_db = init_db("reconcile_and_apply_roll_back_together").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        let key = Key::new(MESSAGE_ID, BRIDGE_ID as BridgeId);
        let mut entries = vec![destination_only_completed_with_transfer()];
        let observations = vec![(
            key,
            vec![
                destination_execution(vec![0xDD], vec![0xAA; 32], Some(1), 20, ts(2_000), "first"),
                destination_execution(vec![0xFE], vec![0xBB; 32], Some(2), 25, ts(2_500), "second"),
            ],
        )];

        let conn = db.db.as_ref();
        let tx = conn.begin().await.unwrap();
        let reconciliation = reconcile_destination_executions(&tx, &mut entries, &observations)
            .await
            .unwrap();
        flush_to_final_storage(&tx, entries).await.unwrap();
        apply_destination_execution_reconciliation(&tx, &reconciliation)
            .await
            .unwrap();
        tx.rollback().await.unwrap();

        assert!(
            crosschain_messages::Entity::find_by_id((MESSAGE_ID, BRIDGE_ID))
                .one(db.db.as_ref())
                .await
                .unwrap()
                .is_none(),
            "a rolled-back transaction must leave no message row"
        );
        assert_eq!(count_destination_execution_anomalies(&db).await, 0);
    }

    /// A stored `Initiated` row -- source already flushed, destination not
    /// yet executed, so `dst_tx_hash` is SQL NULL -- with no
    /// `ConsolidatedMessage` for the key this cycle must **not** be reported
    /// resolved. Before the P2-1 fix, `promotable` was `stored.is_some() ||
    /// consolidated_index.is_some()`, so the mere existence of the stored row
    /// (regardless of `dst_tx_hash`) made the key resolved; the canonical
    /// destination was never written, `pending_messages` would have been
    /// cleared, and the key would have been evicted from hot, losing the
    /// observation permanently.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_reconcile_stored_initiated_row_without_dst_tx_hash_stays_unresolved() {
        let test_db =
            init_db("reconcile_stored_initiated_row_without_dst_tx_hash_stays_unresolved").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_fk_prerequisites(&db).await;

        // Stored row: source side already flushed (`Initiated`), destination
        // side unresolved -- `dst_tx_hash` is SQL NULL, exactly
        // `merge_test_row`'s shape.
        flush(&db, merge_test_row(Some(DST_CHAIN), None)).await;
        assert_eq!(load(&db).await.dst_tx_hash, None, "test setup sanity check");

        // The key is also parked in `pending_messages`, as it would be for a
        // buffer item offloaded to cold storage while waiting on the
        // destination.
        db.upsert_pending_message(pending_messages::ActiveModel {
            message_id: ActiveValue::Set(MESSAGE_ID),
            bridge_id: ActiveValue::Set(BRIDGE_ID),
            payload: ActiveValue::Set(serde_json::Value::Null),
            created_at: ActiveValue::Set(Some(ts(500))),
        })
        .await
        .unwrap();

        let key = Key::new(MESSAGE_ID, BRIDGE_ID as BridgeId);
        // No `ConsolidatedMessage` for this key this cycle: the source side
        // was flushed on an earlier cycle and is not dirty now.
        let mut entries: Vec<ConsolidatedMessage> = Vec::new();
        let observations = vec![(
            key,
            vec![destination_execution(
                vec![0xDD],
                vec![0xAA; 32],
                Some(1),
                20,
                ts(2_000),
                "unresolved stored row",
            )],
        )];

        let conn = db.db.as_ref();
        let tx = conn.begin().await.unwrap();
        let reconciliation = reconcile_destination_executions(&tx, &mut entries, &observations)
            .await
            .unwrap();
        assert!(
            reconciliation.resolved_keys.is_empty(),
            "an Initiated stored row with dst_tx_hash NULL has nowhere for a \
             canonical execution to point yet: the key must not be resolved"
        );
        flush_to_final_storage(&tx, entries).await.unwrap();
        apply_destination_execution_reconciliation(&tx, &reconciliation)
            .await
            .unwrap();
        // Mirrors `commit_maintenance`'s
        // `remove_finalized_from_pending(finalized_keys ++ resolved_keys)`:
        // only keys reported resolved get their pending entry cleared.
        remove_finalized_from_pending(&tx, &reconciliation.resolved_keys)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(count_destination_execution_anomalies(&db).await, 0);
        let row = load(&db).await;
        assert_eq!(row.status, MessageStatus::Initiated);
        assert_eq!(row.dst_tx_hash, None, "the row must remain unresolved");
        assert_eq!(row.protocol_metadata, None);

        assert!(
            pending_messages::Entity::find_by_id((MESSAGE_ID, BRIDGE_ID))
                .one(db.db.as_ref())
                .await
                .unwrap()
                .is_some(),
            "pending_messages must not be cleared for a key that was not resolved"
        );
    }
}
