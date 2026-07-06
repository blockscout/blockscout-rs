// SPDX-License-Identifier: LicenseRef-Blockscout

//! Batch projection of finalized `crosschain_messages` / `crosschain_transfers` into stats tables.
//! Used by the buffer flush (inline) and backfill. All updates for a batch run in one transaction.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use interchain_indexer_entity::{
    bridges, crosschain_messages, crosschain_transfers,
    sea_orm_active_enums::{BridgeType, EdgeAmountSide, MessageStatus},
    stats_asset_edges, stats_asset_tokens, stats_assets, stats_messages, stats_messages_days,
    tokens,
};
use sea_orm::{
    ActiveValue::{Set, Unchanged},
    ColumnTrait, Condition, DatabaseTransaction, DbErr, EntityTrait, JoinType, QueryFilter,
    QuerySelect, RelationTrait,
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
        if let Some(addr) = &t.token_src_address {
            s.insert((t.token_src_chain_id, addr.clone()));
        }
        if let Some(addr) = &t.token_dst_address {
            s.insert((t.token_dst_chain_id, addr.clone()));
        }
    }
    s.into_iter().collect()
}

fn finalized_message_stats_condition() -> Condition {
    Condition::any()
        .add(crosschain_messages::Column::Status.eq(MessageStatus::Completed))
        .add(
            Condition::all()
                .add(crosschain_messages::Column::Status.eq(MessageStatus::Failed))
                .add(bridges::Column::Type.eq(BridgeType::Amb)),
        )
}

/// Project eligible finalized messages into `stats_messages`, `stats_messages_days`,
/// and mark them processed.
/// Eligible: `stats_processed = 0`, `status = completed` (all bridges) or
/// `failed` (AMB only), `dst_chain_id` set.
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
        .join(
            JoinType::InnerJoin,
            crosschain_messages::Relation::Bridges.def(),
        )
        .filter(
            Expr::tuple([
                Expr::col((crosschain_messages::Entity, crosschain_messages::Column::Id)).into(),
                Expr::col((
                    crosschain_messages::Entity,
                    crosschain_messages::Column::BridgeId,
                ))
                .into(),
            ])
            .in_tuples(pks.iter().copied()),
        )
        .filter(
            Expr::col((
                crosschain_messages::Entity,
                crosschain_messages::Column::StatsProcessed,
            ))
            .eq(0i16),
        )
        .filter(finalized_message_stats_condition())
        .filter(
            Expr::col((
                crosschain_messages::Entity,
                crosschain_messages::Column::DstChainId,
            ))
            .is_not_null(),
        )
        .all(tx)
        .await?;

    if rows.is_empty() {
        return Ok(0);
    }

    let mut by_edge: HashMap<(i64, i64), i64> = HashMap::new();
    let mut by_edge_day: HashMap<(chrono::NaiveDate, i64, i64), i64> = HashMap::new();
    for m in &rows {
        let dst = m.dst_chain_id.expect("filtered is_not_null");
        *by_edge.entry((m.src_chain_id, dst)).or_insert(0) += 1;
        *by_edge_day
            .entry((m.init_timestamp.date(), m.src_chain_id, dst))
            .or_insert(0) += 1;
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

    for ((date, src_chain_id, dst_chain_id), messages_delta) in by_edge_day {
        let model = stats_messages_days::ActiveModel {
            date: Set(date),
            src_chain_id: Set(src_chain_id),
            dst_chain_id: Set(dst_chain_id),
            messages_count: Set(messages_delta),
            ..Default::default()
        };
        stats_messages_days::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([
                    stats_messages_days::Column::Date,
                    stats_messages_days::Column::SrcChainId,
                    stats_messages_days::Column::DstChainId,
                ])
                .value(
                    stats_messages_days::Column::MessagesCount,
                    Expr::col((
                        stats_messages_days::Entity,
                        stats_messages_days::Column::MessagesCount,
                    ))
                    .add(messages_delta),
                )
                .value(
                    stats_messages_days::Column::UpdatedAt,
                    Expr::current_timestamp(),
                )
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
            let Some(addr) = &t.token_src_address else {
                continue;
            };
            let ks = (t.token_src_chain_id, addr.clone());
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
            let Some(addr) = &t.token_dst_address else {
                continue;
            };
            let kd = (t.token_dst_chain_id, addr.clone());
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

/// Read the stats asset a chain-local token is already linked to, if any.
///
/// Used to resolve an endpoint before attempting to link it: a failed
/// `INSERT` (e.g. a `UNIQUE (chain_id, token_address)` violation) aborts the
/// whole Postgres transaction, so we must never rely on a failing insert to
/// detect an existing mapping.
async fn lookup_token_asset(
    tx: &DatabaseTransaction,
    chain_id: i64,
    token_address: Vec<u8>,
) -> Result<Option<i64>, DbErr> {
    Ok(stats_asset_tokens::Entity::find()
        .filter(stats_asset_tokens::Column::ChainId.eq(chain_id))
        .filter(stats_asset_tokens::Column::TokenAddress.eq(token_address))
        .one(tx)
        .await?
        .map(|r| r.stats_asset_id))
}

/// Whether a stats asset already has a (necessarily different) token linked on
/// the given chain. `stats_asset_tokens` PK is `(stats_asset_id, chain_id)`, so
/// an asset holds at most one token per chain; checking this via `SELECT` lets
/// us detect an unresolvable conflict without an `INSERT` that would abort the
/// transaction.
async fn asset_has_token_on_chain(
    tx: &DatabaseTransaction,
    stats_asset_id: i64,
    chain_id: i64,
) -> Result<bool, DbErr> {
    Ok(stats_asset_tokens::Entity::find()
        .filter(stats_asset_tokens::Column::StatsAssetId.eq(stats_asset_id))
        .filter(stats_asset_tokens::Column::ChainId.eq(chain_id))
        .one(tx)
        .await?
        .is_some())
}

/// Resolve the stats asset for a transfer's endpoints, linking tokens as needed.
///
/// Returns `Ok(None)` when the endpoints cannot be reconciled to a single asset
/// (e.g. corrupt token data that would map one asset to two tokens on a chain).
/// The caller skips such a transfer's stats projection instead of failing the
/// whole batch — every link is preceded by a `SELECT`, so a conflict is detected
/// without issuing an `INSERT` that would poison the shared maintenance
/// transaction (which also carries message and cursor persistence).
async fn ensure_asset_for_transfer(
    tx: &DatabaseTransaction,
    t: &crosschain_transfers::Model,
    token_to_asset: &mut HashMap<TokenKey, i64>,
) -> Result<Option<i64>, DbErr> {
    // A transfer side whose token is unknown (its bridge event was never
    // observed) contributes no endpoint to reconcile.
    let k_src = t
        .token_src_address
        .clone()
        .map(|addr| (t.token_src_chain_id, addr));
    let k_dst = t
        .token_dst_address
        .clone()
        .map(|addr| (t.token_dst_chain_id, addr));

    // Resolve each present endpoint's existing stats asset before linking:
    // prefer the in-batch cache, then fall back to the persisted mapping.
    let a = match &k_src {
        Some(k) => match token_to_asset.get(k).copied() {
            Some(x) => Some(x),
            None => lookup_token_asset(tx, k.0, k.1.clone()).await?,
        },
        None => None,
    };
    let b = match &k_dst {
        Some(k) => match token_to_asset.get(k).copied() {
            Some(y) => Some(y),
            None => lookup_token_asset(tx, k.0, k.1.clone()).await?,
        },
        None => None,
    };
    if let (Some(k), Some(x)) = (&k_src, a) {
        token_to_asset.insert(k.clone(), x);
    }
    if let (Some(k), Some(y)) = (&k_dst, b) {
        token_to_asset.insert(k.clone(), y);
    }

    let asset_id = match (k_src, k_dst) {
        // Both endpoints known: reconcile them to a single asset.
        (Some(k_src), Some(k_dst)) => match (a, b) {
            (Some(x), Some(y)) if x == y => x,
            (Some(x), Some(y)) => {
                tracing::warn!(
                    transfer_id = t.id,
                    src_stats_asset_id = x,
                    dst_stats_asset_id = y,
                    "stats projection: transfer endpoints map to different stats assets; skipping"
                );
                return Ok(None);
            }
            (Some(x), None) => {
                if asset_has_token_on_chain(tx, x, k_dst.0).await? {
                    tracing::warn!(
                        transfer_id = t.id,
                        stats_asset_id = x,
                        chain_id = k_dst.0,
                        "stats projection: stats asset already has a different token on the destination chain; skipping transfer"
                    );
                    return Ok(None);
                }
                try_link_token(tx, x, k_dst.0, k_dst.1.clone()).await?;
                token_to_asset.insert(k_dst, x);
                x
            }
            (None, Some(y)) => {
                if asset_has_token_on_chain(tx, y, k_src.0).await? {
                    tracing::warn!(
                        transfer_id = t.id,
                        stats_asset_id = y,
                        chain_id = k_src.0,
                        "stats projection: stats asset already has a different token on the source chain; skipping transfer"
                    );
                    return Ok(None);
                }
                try_link_token(tx, y, k_src.0, k_src.1.clone()).await?;
                token_to_asset.insert(k_src, y);
                y
            }
            (None, None) => {
                // A fresh asset can hold at most one token per chain; two distinct
                // tokens on the same chain cannot both link to it.
                if k_src.0 == k_dst.0 && k_src.1 != k_dst.1 {
                    tracing::warn!(
                        transfer_id = t.id,
                        chain_id = k_src.0,
                        "stats projection: transfer endpoints are two different tokens on one chain; skipping"
                    );
                    return Ok(None);
                }
                let id = insert_stats_asset(tx).await?;
                try_link_token(tx, id, k_src.0, k_src.1.clone()).await?;
                // Avoid a duplicate link when both endpoints are the same chain-local token.
                if k_dst != k_src {
                    try_link_token(tx, id, k_dst.0, k_dst.1.clone()).await?;
                }
                token_to_asset.insert(k_src, id);
                token_to_asset.insert(k_dst, id);
                id
            }
        },
        // Only one endpoint known: map to its asset, creating one if needed.
        (Some(k), None) | (None, Some(k)) => match a.or(b) {
            Some(existing) => existing,
            None => {
                let id = insert_stats_asset(tx).await?;
                try_link_token(tx, id, k.0, k.1.clone()).await?;
                token_to_asset.insert(k, id);
                id
            }
        },
        // No token info on either side: nothing to map.
        (None, None) => return Ok(None),
    };

    Ok(Some(asset_id))
}

fn token_decimals(token_rows: &HashMap<TokenKey, tokens::Model>, k: &TokenKey) -> Option<i16> {
    token_rows.get(k).and_then(|m| m.decimals)
}

/// Raw transfer amount for an edge's side, falling back to the opposite side
/// when the requested side is unknown (e.g. a destination-only transfer has no
/// source amount). Defaults to zero only when neither side has an amount.
fn transfer_amount_for_side(
    transfer: &crosschain_transfers::Model,
    amount_side: &EdgeAmountSide,
) -> BigDecimal {
    let (primary, fallback) = match amount_side {
        EdgeAmountSide::Source => (&transfer.src_amount, &transfer.dst_amount),
        EdgeAmountSide::Destination => (&transfer.dst_amount, &transfer.src_amount),
    };
    primary
        .clone()
        .or_else(|| fallback.clone())
        .unwrap_or_else(|| BigDecimal::from(0u64))
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
    let amount = transfer_amount_for_side(transfer, amount_side);
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
/// Eligible: `stats_processed = 0` and parent message `completed` (all bridges)
/// or `failed` (AMB only).
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
            JoinType::InnerJoin,
            crosschain_transfers::Relation::CrosschainMessages.def(),
        )
        .join(
            JoinType::InnerJoin,
            crosschain_messages::Relation::Bridges.def(),
        )
        .filter(finalized_message_stats_condition())
        .filter(
            Expr::col((
                crosschain_transfers::Entity,
                crosschain_transfers::Column::StatsProcessed,
            ))
            .eq(0i16),
        )
        .filter(
            Expr::col((
                crosschain_transfers::Entity,
                crosschain_transfers::Column::Id,
            ))
            .is_in(ids.clone()),
        )
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
        if let Some(addr) = &t.token_src_address {
            pairs.insert((t.token_src_chain_id, addr.clone()));
        }
        if let Some(addr) = &t.token_dst_address {
            pairs.insert((t.token_dst_chain_id, addr.clone()));
        }
        message_keys.insert((t.message_id, t.bridge_id));
    }
    let mut token_to_asset = load_token_asset_map(tx, &pairs).await?;
    let token_rows = load_token_rows_map(tx, &pairs).await?;
    let message_rows = load_message_rows_map(tx, &message_keys).await?;

    use std::collections::hash_map::Entry;

    // Resolve each transfer's stats asset. A transfer whose endpoints cannot be
    // reconciled to a single asset (corrupt token data, conflicting mapping) is
    // skipped rather than aborting the batch — otherwise one bad transfer would
    // roll back the shared maintenance transaction (message + cursor writes)
    // every cycle. Skipped transfers are still marked processed below so they
    // are not retried forever.
    let mut proj_transfers: Vec<crosschain_transfers::Model> = Vec::with_capacity(transfers.len());
    let mut asset_ids: Vec<i64> = Vec::with_capacity(transfers.len());
    let mut edge_key_per_transfer: Vec<EdgeKey> = Vec::with_capacity(transfers.len());
    let mut skipped_ids: Vec<i64> = Vec::new();
    for t in &transfers {
        match ensure_asset_for_transfer(tx, t, &mut token_to_asset).await? {
            Some(asset_id) => {
                edge_key_per_transfer.push((asset_id, t.token_src_chain_id, t.token_dst_chain_id));
                asset_ids.push(asset_id);
                proj_transfers.push(t.clone());
            }
            None => skipped_ids.push(t.id),
        }
    }
    let transfers = proj_transfers;

    let existing_edges = load_stats_asset_edges_for_keys(tx, &edge_key_per_transfer).await?;
    let mut edge_acc: HashMap<EdgeKey, EdgeAccum> = HashMap::new();

    for (t, &asset_id) in transfers.iter().zip(&asset_ids) {
        let edge_key: EdgeKey = (asset_id, t.token_src_chain_id, t.token_dst_chain_id);
        let src_dec = t
            .token_src_address
            .as_ref()
            .and_then(|addr| token_decimals(&token_rows, &(t.token_src_chain_id, addr.clone())));
        let dst_dec = t
            .token_dst_address
            .as_ref()
            .and_then(|addr| token_decimals(&token_rows, &(t.token_dst_chain_id, addr.clone())));
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
                    let (amount_side, decimals) = if source_chain_indexed || src_dec.is_some() {
                        (EdgeAmountSide::Source, src_dec)
                    } else {
                        (EdgeAmountSide::Destination, dst_dec)
                    };
                    let cumulative = transfer_amount_for_side(t, &amount_side);
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

    // Mark conflict-skipped transfers processed (without a stats asset) so the
    // maintenance loop does not reprocess and re-skip them every cycle.
    if !skipped_ids.is_empty() {
        run_in_batches(&skipped_ids, 1, |batch| async {
            crosschain_transfers::Entity::update_many()
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

    Ok(transfers.len() + skipped_ids.len())
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
            src_amount: Some(BigDecimal::from(0u64)),
            dst_amount: Some(BigDecimal::from(0u64)),
            token_src_address: Some(a.clone()),
            token_dst_address: Some(b.clone()),
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
            token_src_address: Some(a.clone()),
            token_dst_address: Some(b.clone()),
            ..t1.clone()
        };
        let keys = token_keys_for_stats_enrichment_from_transfer_models(&[t1, t2]);
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&(1, a)));
        assert!(keys.contains(&(100, b)));
    }
}
