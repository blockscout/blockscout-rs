//! Batch projection of finalized `crosschain_messages` / `crosschain_transfers` into stats tables.
//! Used by the buffer flush (inline) and backfill. All updates for a batch run in one transaction.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use interchain_indexer_entity::{
    crosschain_messages, crosschain_transfers,
    sea_orm_active_enums::{EdgeAmountSide, MessageStatus},
    stats_asset_edges, stats_asset_tokens, stats_assets, stats_messages, tokens,
};
use sea_orm::{
    ActiveValue::{Set, Unchanged},
    ColumnTrait, DatabaseTransaction, DbErr, EntityTrait, QueryFilter, QuerySelect, RelationTrait,
    prelude::BigDecimal,
    sea_query::{Expr, OnConflict},
};

use crate::bulk::run_in_batches;

/// Distinct `(chain_id, token_address)` from transfers — for [`TokenInfoService::kickoff_token_fetch_for_stats_enrichment`]
/// after projection commits (inline flush or backfill).
pub fn token_keys_for_stats_enrichment_from_transfer_models(
    transfers: &[crosschain_transfers::Model],
) -> Vec<(i64, Vec<u8>)> {
    let mut s = HashSet::new();
    for t in transfers {
        s.insert((t.token_src_chain_id, t.token_src_address.clone()));
        s.insert((t.token_dst_chain_id, t.token_dst_address.clone()));
    }
    s.into_iter().collect()
}

/// Project eligible finalized messages into `stats_messages` and mark them processed.
/// Eligible: `stats_processed = 0`, `status = completed`, `dst_chain_id` set.
/// Returns how many message rows were updated.
pub async fn project_messages_batch(
    tx: &DatabaseTransaction,
    message_pks: &[(i64, i32)], // [(message_id, bridge_id)]
) -> Result<usize, DbErr> {
    if message_pks.is_empty() {
        return Ok(0);
    }
    let unique: HashSet<(i64, i32)> = message_pks.iter().copied().collect();
    let pks: Vec<(i64, i32)> = unique.into_iter().collect();

    let rows = crosschain_messages::Entity::find()
        .filter(
            Expr::tuple([
                Expr::col(crosschain_messages::Column::Id).into(),
                Expr::col(crosschain_messages::Column::BridgeId).into(),
            ])
            .in_tuples(pks.iter().copied()),
        )
        .filter(crosschain_messages::Column::StatsProcessed.eq(0i16))
        .filter(crosschain_messages::Column::Status.eq(MessageStatus::Completed))
        .filter(crosschain_messages::Column::DstChainId.is_not_null())
        .all(tx)
        .await?;

    if rows.is_empty() {
        return Ok(0);
    }

    let mut by_edge: HashMap<(i64, i64), i64> = HashMap::new();
    for m in &rows {
        let dst = m.dst_chain_id.expect("filtered is_not_null");
        *by_edge.entry((m.src_chain_id, dst)).or_insert(0) += 1;
    }

    for ((src_chain_id, dst_chain_id), messages_delta) in by_edge {
        let model = stats_messages::ActiveModel {
            src_chain_id: Set(src_chain_id),
            dst_chain_id: Set(dst_chain_id),
            messages_count: Set(messages_delta),
            ..Default::default()
        };
        stats_messages::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([
                    stats_messages::Column::SrcChainId,
                    stats_messages::Column::DstChainId,
                ])
                .value(
                    stats_messages::Column::MessagesCount,
                    Expr::col((
                        stats_messages::Entity,
                        stats_messages::Column::MessagesCount,
                    ))
                    .add(messages_delta),
                )
                .value(stats_messages::Column::UpdatedAt, Expr::current_timestamp())
                .to_owned(),
            )
            .exec(tx)
            .await?;
    }

    let mark: Vec<(i64, i32)> = rows.iter().map(|m| (m.id, m.bridge_id)).collect();
    run_in_batches(&mark, 2, |batch| async {
        crosschain_messages::Entity::update_many()
            .col_expr(
                crosschain_messages::Column::StatsProcessed,
                Expr::col(crosschain_messages::Column::StatsProcessed).add(1),
            )
            .col_expr(
                crosschain_messages::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(
                Expr::tuple([
                    Expr::col(crosschain_messages::Column::Id).into(),
                    Expr::col(crosschain_messages::Column::BridgeId).into(),
                ])
                .in_tuples(batch.iter().copied()),
            )
            .filter(crosschain_messages::Column::StatsProcessed.eq(0i16))
            .exec(tx)
            .await?;
        Ok(())
    })
    .await?;

    Ok(rows.len())
}

// `(chain_id, token_address)`
type TokenKey = (i64, Vec<u8>);

// `(message_id, bridge_id)`
type MessageKey = (i64, i32);

// Returns a map of `TokenKey` to `stats_asset_id`.
async fn load_token_asset_map(
    tx: &DatabaseTransaction,
    pairs: &HashSet<TokenKey>,
) -> Result<HashMap<TokenKey, i64>, DbErr> {
    if pairs.is_empty() {
        return Ok(HashMap::new());
    }
    let list: Vec<TokenKey> = pairs.iter().cloned().collect();
    let batch_size = crate::bulk::PG_BIND_PARAM_LIMIT / 2;
    let mut map = HashMap::new();
    for batch in list.chunks(batch_size.max(1)) {
        let rows = stats_asset_tokens::Entity::find()
            .filter(
                Expr::tuple([
                    Expr::col(stats_asset_tokens::Column::ChainId).into(),
                    Expr::col(stats_asset_tokens::Column::TokenAddress).into(),
                ])
                .in_tuples(batch.iter().map(|(c, a)| (*c, a.clone()))),
            )
            .all(tx)
            .await?;
        for r in rows {
            map.insert((r.chain_id, r.token_address), r.stats_asset_id);
        }
    }
    Ok(map)
}

/// Token rows present in `tokens` for the given keys (missing keys = no row).
async fn load_token_rows_map(
    tx: &DatabaseTransaction,
    pairs: &HashSet<TokenKey>,
) -> Result<HashMap<TokenKey, tokens::Model>, DbErr> {
    if pairs.is_empty() {
        return Ok(HashMap::new());
    }
    let list: Vec<TokenKey> = pairs.iter().cloned().collect();
    let batch_size = crate::bulk::PG_BIND_PARAM_LIMIT / 2;
    let mut map = HashMap::new();
    for batch in list.chunks(batch_size.max(1)) {
        let rows = tokens::Entity::find()
            .filter(
                Expr::tuple([
                    Expr::col(tokens::Column::ChainId).into(),
                    Expr::col(tokens::Column::Address).into(),
                ])
                .in_tuples(batch.iter().map(|(c, a)| (*c, a.clone()))),
            )
            .all(tx)
            .await?;
        for r in rows {
            map.insert((r.chain_id, r.address.clone()), r);
        }
    }
    Ok(map)
}

async fn load_message_rows_map(
    tx: &DatabaseTransaction,
    pairs: &HashSet<MessageKey>,
) -> Result<HashMap<MessageKey, crosschain_messages::Model>, DbErr> {
    if pairs.is_empty() {
        return Ok(HashMap::new());
    }
    let list: Vec<MessageKey> = pairs.iter().copied().collect();
    let batch_size = crate::bulk::PG_BIND_PARAM_LIMIT / 2;
    let mut map = HashMap::new();
    for batch in list.chunks(batch_size.max(1)) {
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
        for r in rows {
            map.insert((r.id, r.bridge_id), r);
        }
    }
    Ok(map)
}

fn non_empty_opt(s: Option<String>) -> Option<String> {
    s.filter(|t| !t.trim().is_empty())
}

/// Fill empty `stats_assets` fields from `tokens` (source token first, then destination).
async fn enrich_stats_assets_for_batch(
    tx: &DatabaseTransaction,
    transfers: &[crosschain_transfers::Model],
    asset_ids: &[i64],
    token_rows: &HashMap<TokenKey, tokens::Model>,
) -> Result<(), DbErr> {
    let mut seen: HashSet<i64> = HashSet::new();
    for &aid in asset_ids {
        if !seen.insert(aid) {
            continue;
        }

        let mut pick_name = None;
        let mut pick_symbol = None;
        let mut pick_icon = None;

        for (t, &a) in transfers.iter().zip(asset_ids.iter()) {
            if a != aid {
                continue;
            }
            let ks = (t.token_src_chain_id, t.token_src_address.clone());
            if let Some(row) = token_rows.get(&ks) {
                if pick_name.is_none() {
                    pick_name = non_empty_opt(row.name.clone());
                }
                if pick_symbol.is_none() {
                    pick_symbol = non_empty_opt(row.symbol.clone());
                }
                if pick_icon.is_none() {
                    pick_icon = non_empty_opt(row.token_icon.clone());
                }
            }
        }
        for (t, &a) in transfers.iter().zip(asset_ids.iter()) {
            if a != aid {
                continue;
            }
            let kd = (t.token_dst_chain_id, t.token_dst_address.clone());
            if let Some(row) = token_rows.get(&kd) {
                if pick_name.is_none() {
                    pick_name = non_empty_opt(row.name.clone());
                }
                if pick_symbol.is_none() {
                    pick_symbol = non_empty_opt(row.symbol.clone());
                }
                if pick_icon.is_none() {
                    pick_icon = non_empty_opt(row.token_icon.clone());
                }
            }
        }

        if pick_name.is_none() && pick_symbol.is_none() && pick_icon.is_none() {
            continue;
        }

        let Some(asset) = stats_assets::Entity::find_by_id(aid).one(tx).await? else {
            continue;
        };

        let empty = |s: &Option<String>| s.as_ref().is_none_or(|t| t.trim().is_empty());
        let mut name = asset.name.clone();
        let mut symbol = asset.symbol.clone();
        let mut icon = asset.icon_url.clone();
        let mut changed = false;

        if empty(&name) && pick_name.is_some() {
            name = pick_name.clone();
            changed = true;
        }
        if empty(&symbol) && pick_symbol.is_some() {
            symbol = pick_symbol.clone();
            changed = true;
        }
        if empty(&icon) && pick_icon.is_some() {
            icon = pick_icon.clone();
            changed = true;
        }

        if changed {
            stats_assets::Entity::update(stats_assets::ActiveModel {
                id: Unchanged(aid),
                name: Set(name),
                symbol: Set(symbol),
                icon_url: Set(icon),
                created_at: Unchanged(asset.created_at),
                updated_at: Set(Utc::now().naive_utc()),
            })
            .exec(tx)
            .await?;
        }
    }
    Ok(())
}

async fn insert_stats_asset(tx: &DatabaseTransaction) -> Result<i64, DbErr> {
    let m = stats_assets::ActiveModel {
        ..Default::default()
    };
    let row = stats_assets::Entity::insert(m)
        .exec_with_returning(tx)
        .await?;
    Ok(row.id)
}

async fn try_link_token(
    tx: &DatabaseTransaction,
    stats_asset_id: i64,
    chain_id: i64,
    token_address: Vec<u8>,
) -> Result<(), DbErr> {
    let model = stats_asset_tokens::ActiveModel {
        stats_asset_id: Set(stats_asset_id),
        chain_id: Set(chain_id),
        token_address: Set(token_address),
        ..Default::default()
    };
    stats_asset_tokens::Entity::insert(model).exec(tx).await?;
    Ok(())
}

async fn ensure_asset_for_transfer(
    tx: &DatabaseTransaction,
    t: &crosschain_transfers::Model,
    token_to_asset: &mut HashMap<TokenKey, i64>,
) -> Result<i64, DbErr> {
    let k_src = (t.token_src_chain_id, t.token_src_address.clone());
    let k_dst = (t.token_dst_chain_id, t.token_dst_address.clone());
    let a = token_to_asset.get(&k_src).copied();
    let b = token_to_asset.get(&k_dst).copied();

    let asset_id = match (a, b) {
        (Some(x), Some(y)) if x == y => x,
        (Some(x), Some(y)) => {
            tracing::warn!(
                transfer_id = t.id,
                src_stats_asset_id = x,
                dst_stats_asset_id = y,
                "stats projection: conflicting stats asset mapping for transfer endpoints"
            );
            return Err(DbErr::Custom(format!(
                "stats projection: transfer {} connects tokens already mapped to different stats assets (stats_asset_id {} vs {}); aborting batch",
                t.id, x, y
            )));
        }
        (Some(x), None) => {
            if try_link_token(tx, x, k_dst.0, k_dst.1.clone())
                .await
                .is_ok()
            {
                token_to_asset.insert(k_dst, x);
            } else if let Some(r) = stats_asset_tokens::Entity::find()
                .filter(stats_asset_tokens::Column::ChainId.eq(k_dst.0))
                .filter(stats_asset_tokens::Column::TokenAddress.eq(k_dst.1.clone()))
                .one(tx)
                .await?
            {
                if r.stats_asset_id != x {
                    return Err(DbErr::Custom(format!(
                        "stats projection: transfer {} destination token already mapped to stats_asset_id {}, expected {}",
                        t.id, r.stats_asset_id, x
                    )));
                }
                token_to_asset.insert(k_dst, x);
            }
            x
        }
        (None, Some(y)) => {
            if try_link_token(tx, y, k_src.0, k_src.1.clone())
                .await
                .is_ok()
            {
                token_to_asset.insert(k_src, y);
            } else if let Some(r) = stats_asset_tokens::Entity::find()
                .filter(stats_asset_tokens::Column::ChainId.eq(k_src.0))
                .filter(stats_asset_tokens::Column::TokenAddress.eq(k_src.1.clone()))
                .one(tx)
                .await?
            {
                if r.stats_asset_id != y {
                    return Err(DbErr::Custom(format!(
                        "stats projection: transfer {} source token already mapped to stats_asset_id {}, expected {}",
                        t.id, r.stats_asset_id, y
                    )));
                }
                token_to_asset.insert(k_src, y);
            }
            y
        }
        (None, None) => {
            let id = insert_stats_asset(tx).await?;
            try_link_token(tx, id, k_src.0, k_src.1.clone()).await?;
            try_link_token(tx, id, k_dst.0, k_dst.1.clone()).await?;
            token_to_asset.insert(k_src, id);
            token_to_asset.insert(k_dst, id);
            id
        }
    };

    Ok(asset_id)
}

fn token_decimals(token_rows: &HashMap<TokenKey, tokens::Model>, k: &TokenKey) -> Option<i16> {
    token_rows.get(k).and_then(|m| m.decimals)
}

type EdgeKey = (i64, i64, i64);

async fn load_stats_asset_edges_for_keys(
    tx: &DatabaseTransaction,
    keys: &[EdgeKey],
) -> Result<HashMap<EdgeKey, stats_asset_edges::Model>, DbErr> {
    if keys.is_empty() {
        return Ok(HashMap::new());
    }
    let mut uniq: HashSet<EdgeKey> = HashSet::new();
    for k in keys {
        uniq.insert(*k);
    }
    let list: Vec<EdgeKey> = uniq.into_iter().collect();
    let batch_size = (crate::bulk::PG_BIND_PARAM_LIMIT / 3).max(1);
    let mut out = HashMap::new();
    for batch in list.chunks(batch_size) {
        let rows = stats_asset_edges::Entity::find()
            .filter(
                Expr::tuple([
                    Expr::col(stats_asset_edges::Column::StatsAssetId).into(),
                    Expr::col(stats_asset_edges::Column::SrcChainId).into(),
                    Expr::col(stats_asset_edges::Column::DstChainId).into(),
                ])
                .in_tuples(batch.iter().copied()),
            )
            .all(tx)
            .await?;
        for r in rows {
            out.insert((r.stats_asset_id, r.src_chain_id, r.dst_chain_id), r);
        }
    }
    Ok(out)
}

fn reject_edge_decimals_mismatch(
    stats_asset_id: i64,
    src_chain_id: i64,
    dst_chain_id: i64,
    stored: i16,
    inc: i16,
) -> DbErr {
    tracing::warn!(
        stats_asset_id,
        src_chain_id,
        dst_chain_id,
        stored,
        incoming = inc,
        "stats projection: rejecting stats_asset_edges update due to decimals mismatch"
    );
    DbErr::Custom(format!(
        "stats_asset_edges ({stats_asset_id},{src_chain_id},{dst_chain_id}): conflicting decimals (stored {stored}, incoming {inc})"
    ))
}

/// Resolves the transfer amount for this edge, updates `working_decimals`, and rejects on mismatch.
fn edge_transfer_amount_for_side(
    amount_side: &EdgeAmountSide,
    working_decimals: &mut Option<i16>,
    transfer: &crosschain_transfers::Model,
    src_decimals: Option<i16>,
    dst_decimals: Option<i16>,
    stats_asset_id: i64,
) -> Result<BigDecimal, DbErr> {
    let amount = match amount_side {
        EdgeAmountSide::Source => transfer.src_amount.clone(),
        EdgeAmountSide::Destination => transfer.dst_amount.clone(),
    };
    let incoming_dec = match amount_side {
        EdgeAmountSide::Source => src_decimals,
        EdgeAmountSide::Destination => dst_decimals,
    };
    if let (Some(stored), Some(inc)) = (*working_decimals, incoming_dec)
        && stored != inc
    {
        return Err(reject_edge_decimals_mismatch(
            stats_asset_id,
            transfer.token_src_chain_id,
            transfer.token_dst_chain_id,
            stored,
            inc,
        ));
    }
    if working_decimals.is_none()
        && let Some(d) = incoming_dec
    {
        *working_decimals = Some(d);
    }
    Ok(amount)
}

/// Per-edge accumulator: mirrors sequential edge updates (sticky `amount_side`, decimals fill).
enum EdgeAccum {
    FromDb {
        db_decimals: Option<i16>,
        working_decimals: Option<i16>,
        amount_side: EdgeAmountSide,
        delta_count: i64,
        delta_amount: BigDecimal,
    },
    NewInBatch {
        amount_side: EdgeAmountSide,
        working_decimals: Option<i16>,
        count: i64,
        cumulative: BigDecimal,
    },
}

impl EdgeAccum {
    fn apply_transfer(
        &mut self,
        stats_asset_id: i64,
        transfer: &crosschain_transfers::Model,
        src_decimals: Option<i16>,
        dst_decimals: Option<i16>,
    ) -> Result<(), DbErr> {
        match self {
            EdgeAccum::FromDb {
                db_decimals: _,
                working_decimals,
                amount_side,
                delta_count,
                delta_amount,
            } => {
                let amount = edge_transfer_amount_for_side(
                    amount_side,
                    working_decimals,
                    transfer,
                    src_decimals,
                    dst_decimals,
                    stats_asset_id,
                )?;
                *delta_count += 1;
                *delta_amount += amount;
                Ok(())
            }
            EdgeAccum::NewInBatch {
                amount_side,
                working_decimals,
                count,
                cumulative,
            } => {
                let amount = edge_transfer_amount_for_side(
                    amount_side,
                    working_decimals,
                    transfer,
                    src_decimals,
                    dst_decimals,
                    stats_asset_id,
                )?;
                *count += 1;
                *cumulative += amount;
                Ok(())
            }
        }
    }
}

/// Project eligible transfers into stats asset tables and mark them processed.
/// Eligible: `stats_processed = 0` and parent message `completed`.
/// Returns how many transfer rows were counted.
pub async fn project_transfers_batch(
    tx: &DatabaseTransaction,
    transfer_ids: &[i64],
) -> Result<usize, DbErr> {
    if transfer_ids.is_empty() {
        return Ok(0);
    }
    let unique_ids: HashSet<i64> = transfer_ids.iter().copied().collect();
    let mut ids: Vec<i64> = unique_ids.into_iter().collect();
    ids.sort_unstable();

    let transfers = crosschain_transfers::Entity::find()
        .join(
            sea_orm::JoinType::InnerJoin,
            crosschain_transfers::Relation::CrosschainMessages.def(),
        )
        .filter(crosschain_messages::Column::Status.eq(MessageStatus::Completed))
        .filter(crosschain_transfers::Column::StatsProcessed.eq(0i16))
        .filter(crosschain_transfers::Column::Id.is_in(ids.clone()))
        .all(tx)
        .await?;

    if transfers.is_empty() {
        return Ok(0);
    }

    let mut transfers = transfers;
    transfers.sort_by_key(|t| t.id);

    let mut pairs: HashSet<TokenKey> = HashSet::new();
    let mut message_keys: HashSet<MessageKey> = HashSet::new();
    for t in &transfers {
        pairs.insert((t.token_src_chain_id, t.token_src_address.clone()));
        pairs.insert((t.token_dst_chain_id, t.token_dst_address.clone()));
        message_keys.insert((t.message_id, t.bridge_id));
    }
    let mut token_to_asset = load_token_asset_map(tx, &pairs).await?;
    let token_rows = load_token_rows_map(tx, &pairs).await?;
    let message_rows = load_message_rows_map(tx, &message_keys).await?;

    use std::collections::hash_map::Entry;

    let mut asset_ids: Vec<i64> = Vec::with_capacity(transfers.len());
    let mut edge_key_per_transfer: Vec<EdgeKey> = Vec::with_capacity(transfers.len());
    for t in &transfers {
        let asset_id = ensure_asset_for_transfer(tx, t, &mut token_to_asset).await?;
        asset_ids.push(asset_id);
        edge_key_per_transfer.push((asset_id, t.token_src_chain_id, t.token_dst_chain_id));
    }

    let existing_edges = load_stats_asset_edges_for_keys(tx, &edge_key_per_transfer).await?;
    let mut edge_acc: HashMap<EdgeKey, EdgeAccum> = HashMap::new();

    for (t, &asset_id) in transfers.iter().zip(&asset_ids) {
        let edge_key: EdgeKey = (asset_id, t.token_src_chain_id, t.token_dst_chain_id);
        let k_src = (t.token_src_chain_id, t.token_src_address.clone());
        let k_dst = (t.token_dst_chain_id, t.token_dst_address.clone());
        let src_dec = token_decimals(&token_rows, &k_src);
        let dst_dec = token_decimals(&token_rows, &k_dst);
        let source_chain_indexed = message_rows
            .get(&(t.message_id, t.bridge_id))
            .is_some_and(|message| message.src_tx_hash.is_some());

        match edge_acc.entry(edge_key) {
            Entry::Vacant(v) => {
                if let Some(edge) = existing_edges.get(&edge_key) {
                    let mut acc = EdgeAccum::FromDb {
                        db_decimals: edge.decimals,
                        working_decimals: edge.decimals,
                        amount_side: edge.amount_side.clone(),
                        delta_count: 0,
                        delta_amount: BigDecimal::from(0u64),
                    };
                    acc.apply_transfer(asset_id, t, src_dec, dst_dec)?;
                    v.insert(acc);
                } else {
                    let (amount_side, cumulative, decimals) =
                        if source_chain_indexed || src_dec.is_some() {
                            (EdgeAmountSide::Source, t.src_amount.clone(), src_dec)
                        } else {
                            (EdgeAmountSide::Destination, t.dst_amount.clone(), dst_dec)
                        };
                    v.insert(EdgeAccum::NewInBatch {
                        amount_side,
                        working_decimals: decimals,
                        count: 1,
                        cumulative,
                    });
                }
            }
            Entry::Occupied(mut o) => {
                o.get_mut().apply_transfer(asset_id, t, src_dec, dst_dec)?;
            }
        }
    }

    for (key, accum) in edge_acc {
        match accum {
            EdgeAccum::FromDb {
                db_decimals,
                working_decimals,
                delta_count,
                delta_amount,
                ..
            } => {
                let (stats_asset_id, src_chain_id, dst_chain_id) = key;
                let mut ub = stats_asset_edges::Entity::update_many()
                    .col_expr(
                        stats_asset_edges::Column::TransfersCount,
                        Expr::col(stats_asset_edges::Column::TransfersCount).add(delta_count),
                    )
                    .col_expr(
                        stats_asset_edges::Column::CumulativeAmount,
                        Expr::col(stats_asset_edges::Column::CumulativeAmount).add(delta_amount),
                    )
                    .col_expr(
                        stats_asset_edges::Column::UpdatedAt,
                        Expr::current_timestamp().into(),
                    )
                    .filter(stats_asset_edges::Column::StatsAssetId.eq(stats_asset_id))
                    .filter(stats_asset_edges::Column::SrcChainId.eq(src_chain_id))
                    .filter(stats_asset_edges::Column::DstChainId.eq(dst_chain_id));
                if db_decimals.is_none()
                    && let Some(d) = working_decimals
                {
                    ub = ub.col_expr(stats_asset_edges::Column::Decimals, Expr::value(d));
                }
                ub.exec(tx).await?;
            }
            EdgeAccum::NewInBatch {
                amount_side,
                working_decimals,
                count,
                cumulative,
            } => {
                let (stats_asset_id, src_chain_id, dst_chain_id) = key;
                stats_asset_edges::Entity::insert(stats_asset_edges::ActiveModel {
                    stats_asset_id: Set(stats_asset_id),
                    src_chain_id: Set(src_chain_id),
                    dst_chain_id: Set(dst_chain_id),
                    transfers_count: Set(count),
                    cumulative_amount: Set(cumulative),
                    decimals: Set(working_decimals),
                    amount_side: Set(amount_side),
                    ..Default::default()
                })
                .exec(tx)
                .await?;
            }
        }
    }

    enrich_stats_assets_for_batch(tx, &transfers, &asset_ids, &token_rows).await?;

    let mut by_asset: HashMap<i64, Vec<i64>> = HashMap::new();
    for (t, &aid) in transfers.iter().zip(&asset_ids) {
        by_asset.entry(aid).or_default().push(t.id);
    }
    for (aid, ids) in by_asset {
        run_in_batches(&ids, 1, |batch| async {
            crosschain_transfers::Entity::update_many()
                .col_expr(crosschain_transfers::Column::StatsAssetId, Expr::value(aid))
                .col_expr(
                    crosschain_transfers::Column::StatsProcessed,
                    Expr::col(crosschain_transfers::Column::StatsProcessed).add(1),
                )
                .col_expr(
                    crosschain_transfers::Column::UpdatedAt,
                    Expr::current_timestamp().into(),
                )
                .filter(crosschain_transfers::Column::Id.is_in(batch.iter().copied()))
                .filter(crosschain_transfers::Column::StatsProcessed.eq(0i16))
                .exec(tx)
                .await?;
            Ok(())
        })
        .await?;
    }

    Ok(transfers.len())
}

#[cfg(test)]
mod token_key_tests {
    use super::token_keys_for_stats_enrichment_from_transfer_models;
    use interchain_indexer_entity::crosschain_transfers;
    use sea_orm::prelude::BigDecimal;

    #[test]
    fn token_keys_from_transfers_includes_src_and_dst_deduped() {
        let a = [0x11u8; 20].to_vec();
        let b = [0x22u8; 20].to_vec();
        let t1 = crosschain_transfers::Model {
            id: 1,
            message_id: 1,
            bridge_id: 1,
            index: 0,
            r#type: None,
            token_src_chain_id: 1,
            token_dst_chain_id: 100,
            src_amount: BigDecimal::from(0u64),
            dst_amount: BigDecimal::from(0u64),
            token_src_address: a.clone(),
            token_dst_address: b.clone(),
            sender_address: None,
            recipient_address: None,
            token_ids: None,
            stats_processed: 0,
            stats_asset_id: None,
            created_at: None,
            updated_at: None,
        };
        let t2 = crosschain_transfers::Model {
            id: 2,
            token_src_address: a.clone(),
            token_dst_address: b.clone(),
            ..t1.clone()
        };
        let keys = token_keys_for_stats_enrichment_from_transfer_models(&[t1, t2]);
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&(1, a)));
        assert!(keys.contains(&(100, b)));
    }
}
