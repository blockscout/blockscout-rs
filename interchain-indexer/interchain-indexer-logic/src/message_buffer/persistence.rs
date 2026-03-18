use std::collections::HashMap;

use alloy::primitives::ChainId;
use interchain_indexer_entity::{
    crosschain_messages, crosschain_transfers, indexer_checkpoints, pending_messages,
};
use itertools::Itertools;
use sea_orm::{
    ActiveValue, ColumnTrait, DatabaseTransaction, DbErr, EntityTrait, Iterable, QueryFilter,
    sea_query::{Expr, OnConflict},
};
use std::collections::HashSet;

use super::{BufferItem, Consolidate, ConsolidatedMessage, Key};
use crate::{
    bulk::{batched_upsert, run_in_batches},
    message_buffer::cursor::{BridgeId, Cursor, CursorBlocksBuilder, Cursors},
};

fn pending_messages_on_conflict() -> OnConflict {
    OnConflict::columns([
        pending_messages::Column::MessageId,
        pending_messages::Column::BridgeId,
    ])
    .update_column(pending_messages::Column::Payload)
    .to_owned()
}

fn crosschain_messages_on_conflict() -> OnConflict {
    OnConflict::columns([
        crosschain_messages::Column::Id,
        crosschain_messages::Column::BridgeId,
    ])
    .update_columns([
        crosschain_messages::Column::Status,
        crosschain_messages::Column::DstChainId,
        crosschain_messages::Column::DstTxHash,
        crosschain_messages::Column::LastUpdateTimestamp,
        crosschain_messages::Column::SenderAddress,
        crosschain_messages::Column::RecipientAddress,
        crosschain_messages::Column::Payload,
    ])
    .to_owned()
}

fn crosschain_transfers_on_conflict() -> OnConflict {
    OnConflict::columns([
        crosschain_transfers::Column::MessageId,
        crosschain_transfers::Column::BridgeId,
        crosschain_transfers::Column::Index,
    ])
    .update_columns(crosschain_transfers::Column::iter().filter(|column| {
        !matches!(
            column,
            crosschain_transfers::Column::Id
                | crosschain_transfers::Column::MessageId
                | crosschain_transfers::Column::BridgeId
                | crosschain_transfers::Column::Index
                | crosschain_transfers::Column::CreatedAt
        )
    }))
    .to_owned()
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

pub(super) async fn flush_to_final_storage(
    tx: &DatabaseTransaction,
    consolidated_entries: Vec<ConsolidatedMessage>,
) -> Result<(), DbErr> {
    let (messages, transfers): (Vec<_>, Vec<_>) = consolidated_entries
        .into_iter()
        .map(|c| (c.message, c.transfers))
        .unzip();
    let transfers = transfers.into_iter().flatten().collect::<Vec<_>>();

    batched_upsert(tx, &messages, crosschain_messages_on_conflict()).await?;
    batched_upsert(tx, &transfers, crosschain_transfers_on_conflict()).await?;

    Ok(())
}

/// Stats for rows that just became final (`is_final`). Must run in the same transaction as
/// `flush_to_final_storage`, after upserts.
pub(super) async fn apply_stats_for_finalized_batch(
    tx: &DatabaseTransaction,
    finalized: &[ConsolidatedMessage],
) -> Result<(), DbErr> {
    if finalized.is_empty() {
        return Ok(());
    }
    let mut msg_pks = Vec::with_capacity(finalized.len());
    for c in finalized {
        let (mid, brid) = match (&c.message.id, &c.message.bridge_id) {
            (ActiveValue::Set(mid), ActiveValue::Set(brid)) => (*mid, *brid),
            _ => {
                return Err(DbErr::Custom(
                    "finalized consolidated message must have id and bridge_id set".into(),
                ));
            }
        };
        msg_pks.push((mid, brid));
    }

    crate::stats_projection::project_messages_batch(tx, &msg_pks).await?;

    let transfer_ids: Vec<i64> = crosschain_transfers::Entity::find()
        .filter(
            Expr::tuple([
                Expr::col(crosschain_transfers::Column::MessageId).into(),
                Expr::col(crosschain_transfers::Column::BridgeId).into(),
            ])
            .in_tuples(msg_pks.iter().copied()),
        )
        .filter(crosschain_transfers::Column::StatsProcessed.eq(0i16))
        .all(tx)
        .await?
        .into_iter()
        .map(|t| t.id)
        .collect();

    crate::stats_projection::project_transfers_batch(tx, &transfer_ids).await?;
    Ok(())
}

/// Distinct `(chain_id, token_address)` from finalized transfers for async token enrichment.
pub(super) fn token_keys_from_finalized_for_enrichment(
    finalized: &[ConsolidatedMessage],
) -> Vec<(i64, Vec<u8>)> {
    let mut out = HashSet::new();
    for c in finalized {
        if !c.is_final {
            continue;
        }
        for t in &c.transfers {
            if let (ActiveValue::Set(sc), ActiveValue::Set(sa)) =
                (&t.token_src_chain_id, &t.token_src_address)
            {
                out.insert((*sc, sa.clone()));
            }
            if let (ActiveValue::Set(dc), ActiveValue::Set(da)) =
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

    // row width = 2 (message_id, bridge_id)
    run_in_batches(&keys, 2, |batch| async {
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
