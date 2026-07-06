// SPDX-License-Identifier: LicenseRef-Blockscout

use std::collections::HashMap;

use alloy::primitives::ChainId;
use interchain_indexer_entity::{
    amb_messages_confirmations, crosschain_messages, crosschain_transfers, indexer_checkpoints,
    pending_messages,
};
use itertools::Itertools;
use sea_orm::{
    ActiveValue, DatabaseTransaction, DbErr, EntityTrait, QueryFilter,
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
        keep_existing_if_terminal("dst_chain_id"),
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
        crosschain_transfers::Column::Type,
        Expr::cust(r#"COALESCE(EXCLUDED."type", crosschain_transfers."type")"#),
    )
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

    run_in_batches(&keys, 2, |batch| async {
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

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDateTime};
    use interchain_indexer_entity::{
        bridges, chains, crosschain_messages, crosschain_transfers,
        sea_orm_active_enums::{MessageStatus, TransferType},
        stats_assets,
    };
    use sea_orm::{
        ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait,
        prelude::BigDecimal,
    };

    use super::{ConsolidatedMessage, flush_to_final_storage};
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
            r#type: ActiveValue::Set(Some(TransferType::Erc20)),
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
            stats_asset_id: ActiveValue::Set(None),
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
        stats_asset_id: Option<i64>,
    ) {
        let transfer = load_transfer(db).await;
        let mut active: crosschain_transfers::ActiveModel = transfer.into();
        active.stats_processed = ActiveValue::Set(stats_processed);
        active.stats_asset_id = ActiveValue::Set(stats_asset_id);
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
        assert_eq!(transfer.stats_asset_id, Some(stats_asset_id));
        assert_eq!(transfer.token_src_address, Some(vec![0xAA]));
        assert_eq!(transfer.token_dst_address, Some(vec![0xBB]));
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
}
