// SPDX-License-Identifier: LicenseRef-Blockscout

//! Batch projection of finalized `crosschain_messages` / `crosschain_transfers` into stats tables.
//! Used by the buffer flush (inline) and backfill. All updates for a batch run in one transaction.

use std::collections::{HashMap, HashSet};

use bigdecimal::RoundingMode;
use chrono::Utc;
use interchain_indexer_entity::{
    bridges, crosschain_messages, crosschain_transfers,
    sea_orm_active_enums::{BridgeType, EdgeAmountSide, MessageStatus, TransferAssetLinkage},
    stats_asset_edges, stats_asset_tokens, stats_assets, stats_messages, stats_messages_days,
    tokens,
};
use sea_orm::{
    ActiveValue::{Set, Unchanged},
    ColumnTrait, Condition, DatabaseTransaction, DbErr, EntityTrait, JoinType, QueryFilter,
    QueryOrder, QuerySelect, RelationTrait,
    prelude::BigDecimal,
    sea_query::{Expr, OnConflict},
};

use crate::bulk::run_in_batches;

use super::{
    indexed_chains::{
        IndexedChains, message_countable_condition, transfer_identity_ready_condition,
    },
    metrics::{
        STATS_ASSET_LINKAGE_CONTRADICTION_TOTAL, STATS_ASSET_MERGE_REPOINTED_TRANSFERS,
        STATS_ASSET_MERGES_TOTAL, STATS_EDGE_DECIMALS_CONFLICT_TOTAL,
        STATS_EDGE_MIXED_AMOUNT_SIDE_TOTAL, STATS_EDGE_RESCALED_FOLD_TOTAL,
        STATS_TRANSFERS_DEFERRED_TOTAL,
    },
};

/// Batch size for repointing `crosschain_transfers.src_stats_asset_id` /
/// `dst_stats_asset_id` during an asset merge. A Rust constant, not a setting
/// — chunking bounds statement size and bind count, not lock duration (the
/// whole merge runs inside the caller's transaction regardless).
const STATS_MERGE_REPOINT_CHUNK: u64 = 5_000;

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

/// Shared stats-eligibility predicate over a `crosschain_messages` row joined to
/// its `bridges` row: a message (or a transfer's parent message) contributes to
/// stats when it is `Completed` (any bridge) or `Failed` on an AMB bridge.
///
/// This is the single source of truth for finality: live projection here and
/// historical backfill candidate selection in `database.rs` both reuse it so
/// they can never diverge (e.g. backfill silently dropping failed AMB rows).
pub(crate) fn finalized_message_stats_condition() -> Condition {
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
/// Eligible: `stats_processed = 0`, `dst_chain_id` set, and countable per
/// [`message_countable_condition`] — confirmed (`status = completed`, any
/// bridge, or `failed` on AMB) or its destination confirmation can never
/// arrive (`dst_chain_id` unindexed for the message's bridge).
/// Returns how many message rows were updated.
pub async fn project_messages_batch(
    tx: &DatabaseTransaction,
    message_pks: &[(i64, i32)], // [(message_id, bridge_id)]
    indexed: &IndexedChains,
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
        .filter(message_countable_condition(indexed))
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

    // Aggregate deltas are bridge-qualified: the same directional chain edge on
    // two different bridges must never be merged into one stats row.
    let mut by_edge: HashMap<(i32, i64, i64), i64> = HashMap::new();
    let mut by_edge_day: HashMap<(chrono::NaiveDate, i32, i64, i64), i64> = HashMap::new();
    for m in &rows {
        let dst = m.dst_chain_id.expect("filtered is_not_null");
        *by_edge
            .entry((m.bridge_id, m.src_chain_id, dst))
            .or_insert(0) += 1;
        *by_edge_day
            .entry((m.init_timestamp.date(), m.bridge_id, m.src_chain_id, dst))
            .or_insert(0) += 1;
    }

    for ((bridge_id, src_chain_id, dst_chain_id), messages_delta) in by_edge {
        let model = stats_messages::ActiveModel {
            bridge_id: Set(bridge_id),
            src_chain_id: Set(src_chain_id),
            dst_chain_id: Set(dst_chain_id),
            messages_count: Set(messages_delta),
            ..Default::default()
        };
        stats_messages::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([
                    stats_messages::Column::BridgeId,
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

    for ((date, bridge_id, src_chain_id, dst_chain_id), messages_delta) in by_edge_day {
        let model = stats_messages_days::ActiveModel {
            date: Set(date),
            bridge_id: Set(bridge_id),
            src_chain_id: Set(src_chain_id),
            dst_chain_id: Set(dst_chain_id),
            messages_count: Set(messages_delta),
            ..Default::default()
        };
        stats_messages_days::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([
                    stats_messages_days::Column::Date,
                    stats_messages_days::Column::BridgeId,
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

/// Fill empty `stats_assets` fields from `tokens`. Two independent passes: a
/// transfer's source token can only ever enrich its `src_asset`, and its
/// destination token only its `dst_asset` — an asset is one token identity, so
/// `name`/`symbol`/`icon_url` must not become first-non-blank-wins across two
/// unrelated tokens on a conversion transfer. For a `mirror` transfer the two
/// ids are equal, and running the source pass fully before the destination
/// pass reproduces today's source-first-then-destination behaviour exactly.
async fn enrich_stats_assets_for_batch(
    tx: &DatabaseTransaction,
    transfers: &[crosschain_transfers::Model],
    asset_id_pairs: &[(i64, i64)],
    token_rows: &HashMap<TokenKey, tokens::Model>,
) -> Result<(), DbErr> {
    enrich_stats_assets_side(
        tx,
        transfers,
        asset_id_pairs,
        token_rows,
        |pair| pair.0,
        |t| {
            t.token_src_address
                .as_ref()
                .map(|addr| (t.token_src_chain_id, addr.clone()))
        },
    )
    .await?;
    enrich_stats_assets_side(
        tx,
        transfers,
        asset_id_pairs,
        token_rows,
        |pair| pair.1,
        |t| {
            t.token_dst_address
                .as_ref()
                .map(|addr| (t.token_dst_chain_id, addr.clone()))
        },
    )
    .await
}

async fn enrich_stats_assets_side(
    tx: &DatabaseTransaction,
    transfers: &[crosschain_transfers::Model],
    asset_id_pairs: &[(i64, i64)],
    token_rows: &HashMap<TokenKey, tokens::Model>,
    side_asset: impl Fn(&(i64, i64)) -> i64,
    side_key: impl Fn(&crosschain_transfers::Model) -> Option<TokenKey>,
) -> Result<(), DbErr> {
    let mut seen: HashSet<i64> = HashSet::new();
    for pair in asset_id_pairs {
        let aid = side_asset(pair);
        if !seen.insert(aid) {
            continue;
        }

        let mut pick_name = None;
        let mut pick_symbol = None;
        let mut pick_icon = None;

        for (t, p) in transfers.iter().zip(asset_id_pairs.iter()) {
            if side_asset(p) != aid {
                continue;
            }
            let Some(key) = side_key(t) else {
                continue;
            };
            if let Some(row) = token_rows.get(&key) {
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

/// Outcome of resolving a transfer's stats asset(s).
enum AssetResolution {
    /// `(src_asset, dst_asset)` — for a `mirror` transfer these are always
    /// equal.
    Resolved(i64, i64),
    /// The endpoints cannot be reconciled (mapping conflict, or a merge
    /// refused on chain collision). The caller marks the transfer processed
    /// with no asset link — identity is genuinely unknown or ambiguous.
    Conflict,
    /// A `conversion` transfer has only one known endpoint, so there is no
    /// second asset to name. The caller must **not** mark this transfer
    /// processed — it is a deferral, not a skip, and is re-evaluated the next
    /// time this transfer's canonical key is flushed.
    Deferred,
}

/// Resolve the stats asset(s) for a transfer's endpoints, linking tokens as
/// needed. Dispatches on `t.asset_linkage`, which the indexer stated when it
/// built the transfer:
///
/// - `mirror` — both endpoints name the same logical asset (today's
///   union-find behaviour, unchanged).
/// - `conversion` — the two endpoints are two different assets by
///   construction (e.g. a converting bridge), resolved independently.
/// - `NULL` — unreachable here: [`transfer_identity_ready_condition`] already
///   excludes rows with an unstated linkage from the query that selects
///   `t`.
async fn ensure_asset_for_transfer(
    tx: &DatabaseTransaction,
    t: &crosschain_transfers::Model,
    token_to_asset: &mut HashMap<TokenKey, i64>,
    merged_away: &mut HashMap<i64, i64>,
    reported_contradiction_pairs: &mut HashSet<(i64, i64)>,
) -> Result<AssetResolution, DbErr> {
    match t.asset_linkage {
        Some(TransferAssetLinkage::Mirror) => Ok(
            match ensure_mirror_asset_for_transfer(
                tx,
                t,
                token_to_asset,
                merged_away,
                reported_contradiction_pairs,
            )
            .await?
            {
                Some(id) => AssetResolution::Resolved(id, id),
                None => AssetResolution::Conflict,
            },
        ),
        Some(TransferAssetLinkage::Conversion) => {
            ensure_conversion_asset_for_transfer(tx, t, token_to_asset).await
        }
        None => {
            tracing::error!(
                transfer_id = t.id,
                "stats projection: transfer reached asset resolution with a NULL \
                 asset_linkage; transfer_identity_ready_condition should have \
                 excluded this row"
            );
            Ok(AssetResolution::Conflict)
        }
    }
}

/// `mirror` resolution: both endpoints name the same logical asset. This is
/// the original single-asset union-find, byte-for-byte unchanged from before
/// per-transfer linkage existed.
///
/// Returns `Ok(None)` when the endpoints cannot be reconciled to a single
/// asset (e.g. corrupt token data that would map one asset to two tokens on a
/// chain, or a merge refused on chain collision). The caller skips such a
/// transfer's stats projection instead of failing the whole batch — every
/// link is preceded by a `SELECT`, so a conflict is detected without issuing
/// an `INSERT` that would poison the shared maintenance transaction (which
/// also carries message and cursor persistence).
///
/// When a transfer's two endpoints resolve to two *different* stats assets,
/// this is not corruption — asset identity is an incrementally discovered
/// connected-component problem, and two complete transfers on fully indexed
/// chains can legitimately form disjoint components that a later transfer
/// bridges. That case is resolved via [`merge_assets`] (a union), not a skip.
async fn ensure_mirror_asset_for_transfer(
    tx: &DatabaseTransaction,
    t: &crosschain_transfers::Model,
    token_to_asset: &mut HashMap<TokenKey, i64>,
    merged_away: &mut HashMap<i64, i64>,
    reported_contradiction_pairs: &mut HashSet<(i64, i64)>,
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
                match merge_assets(
                    tx,
                    x,
                    y,
                    token_to_asset,
                    merged_away,
                    reported_contradiction_pairs,
                )
                .await?
                {
                    Some(winner) => winner,
                    None => return Ok(None),
                }
            }
            (Some(x), None) => {
                if asset_has_token_on_chain(tx, x, k_dst.0).await? {
                    tracing::warn!(
                        transfer_id = t.id,
                        stats_asset_id = x,
                        chain_id = k_dst.0,
                        "stats projection: stats asset already has a different token on the \
                         destination chain; skipping transfer (bad token data, or a converting \
                         route the indexer declared as mirror)"
                    );
                    STATS_ASSET_MERGES_TOTAL
                        .with_label_values(&["refused_token_on_chain"])
                        .inc();
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
                        "stats projection: stats asset already has a different token on the \
                         source chain; skipping transfer (bad token data, or a converting route \
                         the indexer declared as mirror)"
                    );
                    STATS_ASSET_MERGES_TOTAL
                        .with_label_values(&["refused_token_on_chain"])
                        .inc();
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

/// `conversion` resolution: the two endpoints are two different assets by
/// construction, so each side is resolved independently — no
/// `asset_has_token_on_chain` probe and no cross-side link is ever attempted.
/// This path is collision-free by construction: the PK
/// `(stats_asset_id, chain_id)` cannot collide on a freshly created asset, and
/// `UNIQUE (chain_id, token_address)` cannot fire because the lookup precedes
/// the insert.
async fn ensure_conversion_asset_for_transfer(
    tx: &DatabaseTransaction,
    t: &crosschain_transfers::Model,
    token_to_asset: &mut HashMap<TokenKey, i64>,
) -> Result<AssetResolution, DbErr> {
    let src_key = t
        .token_src_address
        .clone()
        .map(|addr| (t.token_src_chain_id, addr));
    let dst_key = t
        .token_dst_address
        .clone()
        .map(|addr| (t.token_dst_chain_id, addr));

    let src_asset = match &src_key {
        Some(k) => Some(resolve_or_create_conversion_endpoint(tx, token_to_asset, k).await?),
        None => None,
    };
    let dst_asset = match &dst_key {
        Some(k) => Some(resolve_or_create_conversion_endpoint(tx, token_to_asset, k).await?),
        None => None,
    };

    match (src_asset, dst_asset) {
        (Some(s), Some(d)) => Ok(AssetResolution::Resolved(s, d)),
        // No token info on either side: nothing to map — mirrors the mirror
        // path's (None, None) arm. Unreachable in practice:
        // `transfer_identity_ready_condition`'s `at_least_one_known` clause
        // already excludes this row.
        (None, None) => Ok(AssetResolution::Conflict),
        // Exactly one endpoint known: there is no second asset to name.
        // Writing src = dst would assert precisely the identity `conversion`
        // denies, so defer instead of guessing.
        _ => {
            STATS_TRANSFERS_DEFERRED_TOTAL
                .with_label_values(&["conversion_endpoint_unresolved"])
                .inc();
            Ok(AssetResolution::Deferred)
        }
    }
}

async fn resolve_or_create_conversion_endpoint(
    tx: &DatabaseTransaction,
    token_to_asset: &mut HashMap<TokenKey, i64>,
    key: &TokenKey,
) -> Result<i64, DbErr> {
    if let Some(&id) = token_to_asset.get(key) {
        return Ok(id);
    }
    if let Some(id) = lookup_token_asset(tx, key.0, key.1.clone()).await? {
        token_to_asset.insert(key.clone(), id);
        return Ok(id);
    }
    let id = insert_stats_asset(tx).await?;
    try_link_token(tx, id, key.0, key.1.clone()).await?;
    token_to_asset.insert(key.clone(), id);
    Ok(id)
}

/// Follows `merged_away` (loser -> winner) to the current owner of `id`.
///
/// A winner can itself become the loser of a later merge within the same
/// batch, so a single hop is not enough — the remap must be transitive. The
/// iteration cap is defensive only: the union-find algorithm here can never
/// produce a cycle (a loser is deleted the moment it is recorded), so hitting
/// the cap indicates a bug, not a legitimate long chain.
fn resolve_merged(merged_away: &HashMap<i64, i64>, id: i64) -> i64 {
    const MAX_HOPS: usize = 64;
    let mut current = id;
    for _ in 0..MAX_HOPS {
        match merged_away.get(&current) {
            Some(&next) => current = next,
            None => return current,
        }
    }
    tracing::error!(
        start_id = id,
        "stats projection: resolve_merged exceeded iteration cap; possible cycle in merged_away"
    );
    current
}

/// Rescale a `NUMERIC(78,0)` cumulative amount from `from_decimals` to
/// `to_decimals` (task Decision 6). Returns `None` when the rescaled result
/// would exceed `NUMERIC(78,0)` (more than 78 digits); the caller falls back
/// to an unscaled add in that case.
fn rescale_edge_amount(
    amount: &BigDecimal,
    from_decimals: i16,
    to_decimals: i16,
) -> Option<BigDecimal> {
    let diff = to_decimals as i64 - from_decimals as i64;
    let scaled = match diff.cmp(&0) {
        std::cmp::Ordering::Equal => amount.clone(),
        std::cmp::Ordering::Greater => amount.clone() * BigDecimal::from(10u64).powi(diff),
        std::cmp::Ordering::Less => {
            let divisor = BigDecimal::from(10u64).powi(-diff);
            (amount.clone() / divisor).with_scale_round(0, RoundingMode::Down)
        }
    };
    if scaled.digits() > 78 {
        None
    } else {
        Some(scaled)
    }
}

/// Merge two stats asset components into one connected component.
///
/// Runs entirely inside the caller's transaction, in two passes:
/// **pass 1 validates** (SELECT only) so the only refusal (a genuine chain
/// collision) is detected before any write happens; **pass 2 mutates**, in a
/// fixed order (tokens -> edges -> transfers -> metadata -> delete the loser),
/// because the FK cascades on `stats_asset_tokens` / `stats_asset_edges`
/// (`ON DELETE CASCADE`) and `crosschain_transfers` (`ON DELETE SET NULL`)
/// make an early delete a silent data-loss bug rather than a visible failure.
///
/// Returns the surviving asset id, or `None` when the merge is refused — the
/// caller then treats the transfer like any other unresolvable conflict
/// (skip, mark processed, keep no link for a not-yet-counted row / keep the
/// existing link for an already-counted repair-path row).
async fn merge_assets(
    tx: &DatabaseTransaction,
    a: i64,
    b: i64,
    token_to_asset: &mut HashMap<TokenKey, i64>,
    merged_away: &mut HashMap<i64, i64>,
    reported_contradiction_pairs: &mut HashSet<(i64, i64)>,
) -> Result<Option<i64>, DbErr> {
    // --- Pass 1: validate (SELECT only) ---

    let tokens = stats_asset_tokens::Entity::find()
        .filter(stats_asset_tokens::Column::StatsAssetId.is_in([a, b]))
        .all(tx)
        .await?;

    let mut chains_a: HashSet<i64> = HashSet::new();
    let mut chains_b: HashSet<i64> = HashSet::new();
    for t in &tokens {
        if t.stats_asset_id == a {
            chains_a.insert(t.chain_id);
        } else {
            chains_b.insert(t.chain_id);
        }
    }

    // `UNIQUE (chain_id, token_address)` means one chain-local token maps to at
    // most one asset, so any chain overlap between the two components is
    // necessarily two *different* tokens on that chain — the genuine conflict.
    if let Some(&collision_chain) = chains_a.intersection(&chains_b).next() {
        tracing::warn!(
            stats_asset_id_a = a,
            stats_asset_id_b = b,
            chain_id = collision_chain,
            "stats projection: refusing asset merge, both components hold a token on the same \
             chain (bad token data, or a converting route the indexer declared as mirror)"
        );
        STATS_ASSET_MERGES_TOTAL
            .with_label_values(&["refused_chain_collision"])
            .inc();
        return Ok(None);
    }

    // Weighted union: the larger component (by linked token count) wins; ties
    // go to the lower id. The winner must be known before folding edges below,
    // since the target scale for rescaling is always the winner's.
    let (winner, loser, loser_token_count) = match chains_a.len().cmp(&chains_b.len()) {
        std::cmp::Ordering::Greater => (a, b, chains_b.len()),
        std::cmp::Ordering::Less => (b, a, chains_a.len()),
        std::cmp::Ordering::Equal => {
            if a <= b {
                (a, b, chains_b.len())
            } else {
                (b, a, chains_a.len())
            }
        }
    };

    // Every edge touching either component: both sides are captured because
    // `winner` is itself `a` or `b`, so a pre-existing winner-only edge and
    // every loser-touching edge (self, one-sided, or a direct cross edge
    // between the two components) are all fetched by this one filter.
    let edges = stats_asset_edges::Entity::find()
        .filter(
            Condition::any()
                .add(stats_asset_edges::Column::SrcStatsAssetId.is_in([a, b]))
                .add(stats_asset_edges::Column::DstStatsAssetId.is_in([a, b])),
        )
        .all(tx)
        .await?;

    // Group by the post-merge key (loser -> winner on both columns). A group
    // with more than one row is a collision that must fold into one row; see
    // the three shapes in the module docs / ADR-011.
    let map_component = |x: i64| if x == loser { winner } else { x };
    type PostMergeKey = (i64, i64, i32, i64, i64);
    let mut groups: HashMap<PostMergeKey, Vec<stats_asset_edges::Model>> = HashMap::new();
    for e in edges {
        let key = (
            map_component(e.src_stats_asset_id),
            map_component(e.dst_stats_asset_id),
            e.bridge_id,
            e.src_chain_id,
            e.dst_chain_id,
        );
        groups.entry(key).or_default().push(e);
    }

    // --- Pass 2: mutate, strictly ordered ---

    // 1. Tokens. The PK `(stats_asset_id, chain_id)` cannot collide — the
    // chain-set intersection check above already proved it.
    stats_asset_tokens::Entity::update_many()
        .col_expr(
            stats_asset_tokens::Column::StatsAssetId,
            Expr::value(winner),
        )
        .col_expr(
            stats_asset_tokens::Column::UpdatedAt,
            Expr::current_timestamp().into(),
        )
        .filter(stats_asset_tokens::Column::StatsAssetId.eq(loser))
        .exec(tx)
        .await?;

    // 2. Edges: for each post-merge key, fold every colliding row into one
    // target row and delete the rest. A group of one row whose key is already
    // its post-merge key needs no write at all (an edge untouched by this
    // merge, fetched only because one column happened to equal the winner).
    let mut edges_folded = 0usize;
    for (post_key, mut group) in groups {
        if group.len() == 1
            && (group[0].src_stats_asset_id, group[0].dst_stats_asset_id)
                == (post_key.0, post_key.1)
        {
            continue;
        }

        // Contradiction guard 2: a pre-existing edge directly between the two
        // components being merged (a real cross-asset route, e.g. an earlier
        // `conversion` transfer) collapsing onto a self-edge because a
        // `mirror` transfer just joined the same two components.
        let is_cross_pair = |e: &stats_asset_edges::Model| {
            (e.src_stats_asset_id == a && e.dst_stats_asset_id == b)
                || (e.src_stats_asset_id == b && e.dst_stats_asset_id == a)
        };
        if post_key.0 == post_key.1 && group.iter().any(is_cross_pair) {
            tracing::warn!(
                winner_stats_asset_id = winner,
                loser_stats_asset_id = loser,
                bridge_id = post_key.2,
                src_chain_id = post_key.3,
                dst_chain_id = post_key.4,
                "stats projection: folding a cross-asset edge into a self-edge; the two assets \
                 were declared both converting and mirrored"
            );
            STATS_ASSET_LINKAGE_CONTRADICTION_TOTAL
                .with_label_values(&["cross_asset_edge_collapsed"])
                .inc();
            reported_contradiction_pairs.insert((post_key.0, post_key.1));
        }

        edges_folded += group.len().saturating_sub(1);

        // Deterministic target: prefer the row already at the post-merge key
        // (no repoint needed for it), else the earliest-created row.
        group.sort_by_key(|e| e.created_at);
        let target_idx = group
            .iter()
            .position(|e| (e.src_stats_asset_id, e.dst_stats_asset_id) == (post_key.0, post_key.1))
            .unwrap_or(0);
        let target = group.remove(target_idx);

        let mut acc_count = target.transfers_count;
        let mut acc_amount = target.cumulative_amount.clone();
        let mut acc_decimals = target.decimals;
        let acc_side = target.amount_side.clone();

        for loser_edge in &group {
            let (add_amount, mode) = match (acc_decimals, loser_edge.decimals) {
                (Some(dw), Some(dl)) if dw != dl => {
                    match rescale_edge_amount(&loser_edge.cumulative_amount, dl, dw) {
                        Some(scaled) => (
                            scaled,
                            Some(if dw > dl { "scaled_up" } else { "scaled_down" }),
                        ),
                        None => (
                            loser_edge.cumulative_amount.clone(),
                            Some("unscaled_overflow"),
                        ),
                    }
                }
                (Some(_), Some(_)) => (loser_edge.cumulative_amount.clone(), None),
                _ => (
                    loser_edge.cumulative_amount.clone(),
                    Some("unscaled_unknown_decimals"),
                ),
            };
            if let Some(mode) = mode {
                tracing::warn!(
                    stats_asset_id = winner,
                    bridge_id = post_key.2,
                    src_chain_id = post_key.3,
                    dst_chain_id = post_key.4,
                    acc_decimals = ?acc_decimals,
                    loser_decimals = ?loser_edge.decimals,
                    scaled = mode,
                    "stats projection: rescaling folded edge amount"
                );
                STATS_EDGE_RESCALED_FOLD_TOTAL
                    .with_label_values(&[mode])
                    .inc();
            }

            if acc_side != loser_edge.amount_side {
                tracing::warn!(
                    stats_asset_id = winner,
                    bridge_id = post_key.2,
                    src_chain_id = post_key.3,
                    dst_chain_id = post_key.4,
                    acc_side = ?acc_side,
                    loser_side = ?loser_edge.amount_side,
                    "stats projection: folding edge rows with different amount_side; cumulative amount is approximate"
                );
                STATS_EDGE_MIXED_AMOUNT_SIDE_TOTAL.inc();
            }

            if acc_decimals.is_none() {
                acc_decimals = loser_edge.decimals;
            }
            acc_count += loser_edge.transfers_count;
            acc_amount += add_amount;
        }

        let mut ub = stats_asset_edges::Entity::update_many()
            .col_expr(
                stats_asset_edges::Column::TransfersCount,
                Expr::value(acc_count),
            )
            .col_expr(
                stats_asset_edges::Column::CumulativeAmount,
                Expr::value(acc_amount),
            )
            .col_expr(
                stats_asset_edges::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(stats_asset_edges::Column::SrcStatsAssetId.eq(target.src_stats_asset_id))
            .filter(stats_asset_edges::Column::DstStatsAssetId.eq(target.dst_stats_asset_id))
            .filter(stats_asset_edges::Column::BridgeId.eq(target.bridge_id))
            .filter(stats_asset_edges::Column::SrcChainId.eq(target.src_chain_id))
            .filter(stats_asset_edges::Column::DstChainId.eq(target.dst_chain_id));
        if target.decimals.is_none() && acc_decimals.is_some() {
            ub = ub.col_expr(
                stats_asset_edges::Column::Decimals,
                Expr::value(acc_decimals),
            );
        }
        if (target.src_stats_asset_id, target.dst_stats_asset_id) != (post_key.0, post_key.1) {
            ub = ub
                .col_expr(
                    stats_asset_edges::Column::SrcStatsAssetId,
                    Expr::value(post_key.0),
                )
                .col_expr(
                    stats_asset_edges::Column::DstStatsAssetId,
                    Expr::value(post_key.1),
                );
        }
        ub.exec(tx).await?;

        for e in &group {
            stats_asset_edges::Entity::delete_many()
                .filter(stats_asset_edges::Column::SrcStatsAssetId.eq(e.src_stats_asset_id))
                .filter(stats_asset_edges::Column::DstStatsAssetId.eq(e.dst_stats_asset_id))
                .filter(stats_asset_edges::Column::BridgeId.eq(e.bridge_id))
                .filter(stats_asset_edges::Column::SrcChainId.eq(e.src_chain_id))
                .filter(stats_asset_edges::Column::DstChainId.eq(e.dst_chain_id))
                .exec(tx)
                .await?;
        }
    }

    // 3. Transfers, chunked. Self-terminating: each pass removes its rows from
    // the predicate (a row with the loser on both sides is fixed in one pass
    // by the `CASE` expressions below). Never touches `stats_processed`.
    let mut repointed: u64 = 0;
    loop {
        let ids: Vec<i64> = crosschain_transfers::Entity::find()
            .select_only()
            .column(crosschain_transfers::Column::Id)
            .filter(
                Condition::any()
                    .add(crosschain_transfers::Column::SrcStatsAssetId.eq(loser))
                    .add(crosschain_transfers::Column::DstStatsAssetId.eq(loser)),
            )
            .order_by_asc(crosschain_transfers::Column::Id)
            .limit(STATS_MERGE_REPOINT_CHUNK)
            .into_tuple()
            .all(tx)
            .await?;
        if ids.is_empty() {
            break;
        }
        repointed += ids.len() as u64;
        run_in_batches(&ids, 1, |batch| async {
            crosschain_transfers::Entity::update_many()
                .col_expr(
                    crosschain_transfers::Column::SrcStatsAssetId,
                    Expr::cust(format!(
                        "CASE WHEN src_stats_asset_id = {loser} THEN {winner} ELSE src_stats_asset_id END"
                    )),
                )
                .col_expr(
                    crosschain_transfers::Column::DstStatsAssetId,
                    Expr::cust(format!(
                        "CASE WHEN dst_stats_asset_id = {loser} THEN {winner} ELSE dst_stats_asset_id END"
                    )),
                )
                .col_expr(
                    crosschain_transfers::Column::UpdatedAt,
                    Expr::current_timestamp().into(),
                )
                .filter(crosschain_transfers::Column::Id.is_in(batch.iter().copied()))
                .exec(tx)
                .await?;
            Ok(())
        })
        .await?;
    }

    // 4. Metadata: fill the winner's empty name/symbol/icon from the loser.
    let winner_asset = stats_assets::Entity::find_by_id(winner).one(tx).await?;
    let loser_asset = stats_assets::Entity::find_by_id(loser).one(tx).await?;
    if let (Some(winner_asset), Some(loser_asset)) = (winner_asset, loser_asset) {
        let empty = |s: &Option<String>| s.as_ref().is_none_or(|t| t.trim().is_empty());
        let mut name = winner_asset.name.clone();
        let mut symbol = winner_asset.symbol.clone();
        let mut icon = winner_asset.icon_url.clone();
        let mut changed = false;
        if empty(&name)
            && let Some(v) = non_empty_opt(loser_asset.name.clone())
        {
            name = Some(v);
            changed = true;
        }
        if empty(&symbol)
            && let Some(v) = non_empty_opt(loser_asset.symbol.clone())
        {
            symbol = Some(v);
            changed = true;
        }
        if empty(&icon)
            && let Some(v) = non_empty_opt(loser_asset.icon_url.clone())
        {
            icon = Some(v);
            changed = true;
        }
        if changed {
            stats_assets::Entity::update(stats_assets::ActiveModel {
                id: Unchanged(winner),
                name: Set(name),
                symbol: Set(symbol),
                icon_url: Set(icon),
                created_at: Unchanged(winner_asset.created_at),
                updated_at: Set(Utc::now().naive_utc()),
            })
            .exec(tx)
            .await?;
        }
    }

    // 5. Delete the loser. By now the cascades have nothing left to destroy.
    stats_assets::Entity::delete_by_id(loser).exec(tx).await?;

    // 6. Fix in-memory batch state so later resolutions in this same batch
    // never reference the now-deleted loser id.
    merged_away.insert(loser, winner);
    for v in token_to_asset.values_mut() {
        if *v == loser {
            *v = winner;
        }
    }

    STATS_ASSET_MERGES_TOTAL
        .with_label_values(&["merged"])
        .inc();
    STATS_ASSET_MERGE_REPOINTED_TRANSFERS.observe(repointed as f64);
    tracing::info!(
        winner_stats_asset_id = winner,
        loser_stats_asset_id = loser,
        tokens_moved = loser_token_count,
        edges_folded,
        transfers_repointed = repointed,
        "stats projection: merged asset components"
    );

    Ok(Some(winner))
}

fn token_decimals(token_rows: &HashMap<TokenKey, tokens::Model>, k: &TokenKey) -> Option<i16> {
    token_rows.get(k).and_then(|m| m.decimals)
}

/// Raw transfer amount for an edge's side.
///
/// For a `mirror` transfer, falls back to the opposite side's raw amount when
/// the chosen side's amount is `NULL` (e.g. a destination-only transfer has no
/// source amount) and defaults to zero only when neither side has an amount —
/// the fee-difference approximation ADR-004 accepts, since both sides are the
/// same asset.
///
/// For a `conversion` transfer the two sides are *different* assets, so no
/// fallback is applied and no zero default is substituted: `None` means "this
/// transfer cannot be safely counted from this side yet", which the caller
/// turns into a deferral (`amount_side_missing`), not a zero.
fn transfer_amount_for_side(
    transfer: &crosschain_transfers::Model,
    amount_side: &EdgeAmountSide,
) -> Option<BigDecimal> {
    let (primary, fallback) = match amount_side {
        EdgeAmountSide::Source => (&transfer.src_amount, &transfer.dst_amount),
        EdgeAmountSide::Destination => (&transfer.dst_amount, &transfer.src_amount),
    };
    if transfer.asset_linkage == Some(TransferAssetLinkage::Conversion) {
        primary.clone()
    } else {
        Some(
            primary
                .clone()
                .or_else(|| fallback.clone())
                .unwrap_or_else(|| BigDecimal::from(0u64)),
        )
    }
}

// `(src_stats_asset_id, dst_stats_asset_id, bridge_id, src_chain_id, dst_chain_id)`
// — edges are bridge-qualified and now binary: the same logical asset pair
// moving over the same chain edge on two different bridges is two distinct
// rows, and a mirror transfer's pair always has `src == dst`.
type EdgeKey = (i64, i64, i32, i64, i64);

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
    // Five bind params per tuple now that the key carries both asset columns.
    let batch_size = (crate::bulk::PG_BIND_PARAM_LIMIT / 5).max(1);
    let mut out = HashMap::new();
    for batch in list.chunks(batch_size) {
        let rows = stats_asset_edges::Entity::find()
            .filter(
                Expr::tuple([
                    Expr::col(stats_asset_edges::Column::SrcStatsAssetId).into(),
                    Expr::col(stats_asset_edges::Column::DstStatsAssetId).into(),
                    Expr::col(stats_asset_edges::Column::BridgeId).into(),
                    Expr::col(stats_asset_edges::Column::SrcChainId).into(),
                    Expr::col(stats_asset_edges::Column::DstChainId).into(),
                ])
                .in_tuples(batch.iter().copied()),
            )
            .all(tx)
            .await?;
        for r in rows {
            out.insert(
                (
                    r.src_stats_asset_id,
                    r.dst_stats_asset_id,
                    r.bridge_id,
                    r.src_chain_id,
                    r.dst_chain_id,
                ),
                r,
            );
        }
    }
    Ok(out)
}

/// Marker for "this transfer's decimals disagree with the edge's already-known
/// decimals". Deliberately *not* a `DbErr` (task Decision 7): the non-merge
/// counting path downgrades this from an aborting error to a per-transfer
/// skip, because a `DbErr` here would roll back the shared maintenance
/// transaction — including cursor writes — every cycle, a poison pill from one
/// bad pair of edge rows. The caller marks the transfer processed without an
/// edge contribution instead. Unlike a genuine mapping conflict, this
/// transfer's asset identity was already resolved unambiguously before this
/// marker fires, so the caller still links `stats_asset_id` to it — only the
/// edge/count contribution is skipped.
#[derive(Debug)]
struct DecimalsConflict;

fn warn_edge_decimals_mismatch(
    src_stats_asset_id: i64,
    dst_stats_asset_id: i64,
    bridge_id: i32,
    src_chain_id: i64,
    dst_chain_id: i64,
    stored: i16,
    inc: i16,
) -> DecimalsConflict {
    tracing::warn!(
        src_stats_asset_id,
        dst_stats_asset_id,
        bridge_id,
        src_chain_id,
        dst_chain_id,
        stored,
        incoming = inc,
        "stats projection: skipping transfer due to stats_asset_edges decimals mismatch"
    );
    STATS_EDGE_DECIMALS_CONFLICT_TOTAL.inc();
    DecimalsConflict
}

/// Resolves the transfer amount for this edge, updates `working_decimals`, and
/// flags a mismatch instead of failing the batch. The caller must have already
/// excluded the `conversion` "amount side missing" deferral — this always
/// resolves to a concrete amount.
fn edge_transfer_amount_for_side(
    amount_side: &EdgeAmountSide,
    working_decimals: &mut Option<i16>,
    transfer: &crosschain_transfers::Model,
    src_decimals: Option<i16>,
    dst_decimals: Option<i16>,
    src_stats_asset_id: i64,
    dst_stats_asset_id: i64,
) -> Result<BigDecimal, DecimalsConflict> {
    let amount =
        transfer_amount_for_side(transfer, amount_side).unwrap_or_else(|| BigDecimal::from(0u64));
    let incoming_dec = match amount_side {
        EdgeAmountSide::Source => src_decimals,
        EdgeAmountSide::Destination => dst_decimals,
    };
    if let (Some(stored), Some(inc)) = (*working_decimals, incoming_dec)
        && stored != inc
    {
        return Err(warn_edge_decimals_mismatch(
            src_stats_asset_id,
            dst_stats_asset_id,
            transfer.bridge_id,
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
    fn amount_side(&self) -> &EdgeAmountSide {
        match self {
            EdgeAccum::FromDb { amount_side, .. } => amount_side,
            EdgeAccum::NewInBatch { amount_side, .. } => amount_side,
        }
    }

    /// Applies one transfer's amount to this accumulator. `Err(DecimalsConflict)`
    /// means the caller must skip this specific transfer (mark it processed,
    /// no edge contribution) rather than abort — see [`DecimalsConflict`].
    /// The caller must have already excluded the `conversion`
    /// "amount side missing" deferral for this transfer.
    fn apply_transfer(
        &mut self,
        src_stats_asset_id: i64,
        dst_stats_asset_id: i64,
        transfer: &crosschain_transfers::Model,
        src_decimals: Option<i16>,
        dst_decimals: Option<i16>,
    ) -> Result<(), DecimalsConflict> {
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
                    src_stats_asset_id,
                    dst_stats_asset_id,
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
                    src_stats_asset_id,
                    dst_stats_asset_id,
                )?;
                *count += 1;
                *cumulative += amount;
                Ok(())
            }
        }
    }
}

/// Pure classifier for why a candidate transfer was not returned by the
/// identity-ready query — used only for the `STATS_TRANSFERS_DEFERRED_TOTAL`
/// metric label, kept separate from the SQL predicate logic so it can be unit
/// tested without a database or a shared process-wide counter.
///
/// `linkage_unknown` is checked **first**: a row with both an unknown linkage
/// and a missing endpoint cannot be resolved by the endpoint arriving on its
/// own, so reporting `identity_incomplete` for it would point an operator at
/// the wrong thing.
fn deferral_reason(t: &crosschain_transfers::Model, indexed: &IndexedChains) -> &'static str {
    if t.asset_linkage.is_none() {
        return "linkage_unknown";
    }
    let identity_incomplete = (t.token_src_address.is_none()
        && indexed.may_observe(t.bridge_id, t.token_src_chain_id))
        || (t.token_dst_address.is_none()
            && indexed.may_observe(t.bridge_id, t.token_dst_chain_id));
    if identity_incomplete {
        "identity_incomplete"
    } else {
        "awaiting_confirmation"
    }
}

/// Project eligible transfers into stats asset tables and mark them processed.
///
/// Selection follows the module-level rule that separates counting from
/// identity maintenance: a row is returned when it is
/// identity-ready per [`transfer_identity_ready_condition`] (every unknown
/// token endpoint sits on a chain unindexed for this bridge, and at least one
/// endpoint is known) **and** either it was already counted
/// (`stats_processed > 0`, the repair path) or it is newly countable
/// (`stats_processed = 0` and [`message_countable_condition`] holds).
///
/// Newly countable rows go through the full counting path: asset resolution
/// (a union, possibly merging two components — see [`merge_assets`]), edge
/// accumulation, and marking `stats_processed` from 0 to 1. Already-counted
/// rows only re-run asset resolution (idempotent identity maintenance for a
/// transfer whose endpoints changed since it was counted, e.g. a newly indexed
/// chain filled a previously missing side) — additive aggregates and
/// `stats_processed` are never touched for these.
///
/// Returns how many transfer rows were newly counted or newly conflict-skipped
/// (i.e. how many rows transitioned `stats_processed` from 0 to 1); repair-only
/// rows are not counted in the return value.
pub async fn project_transfers_batch(
    tx: &DatabaseTransaction,
    transfer_ids: &[i64],
    indexed: &IndexedChains,
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
        .filter(transfer_identity_ready_condition(indexed))
        .filter(
            Condition::any()
                .add(
                    Expr::col((
                        crosschain_transfers::Entity,
                        crosschain_transfers::Column::StatsProcessed,
                    ))
                    .gt(0i16),
                )
                .add(
                    Condition::all()
                        .add(
                            Expr::col((
                                crosschain_transfers::Entity,
                                crosschain_transfers::Column::StatsProcessed,
                            ))
                            .eq(0i16),
                        )
                        .add(message_countable_condition(indexed)),
                ),
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

    // Deferral bookkeeping: any requested id that the eligibility filters above
    // did not return is deferred, not lost — it stays `stats_processed = 0` and
    // will be re-evaluated the next time its canonical key is flushed. Classify
    // by re-checking `IndexedChains::may_observe` in Rust so the metric label
    // stays honest without duplicating the SQL predicate logic.
    if transfer_ids.len() > transfers.len() {
        let projected_ids: HashSet<i64> = transfers.iter().map(|t| t.id).collect();
        let deferred_ids: Vec<i64> = ids
            .iter()
            .copied()
            .filter(|id| !projected_ids.contains(id))
            .collect();
        if !deferred_ids.is_empty() {
            let deferred_rows = crosschain_transfers::Entity::find()
                .filter(crosschain_transfers::Column::Id.is_in(deferred_ids))
                // Exclude rows a concurrent writer already finished between the
                // caller's initial candidate selection and this transaction:
                // those are done, not deferred, and must not be metric-counted.
                .filter(crosschain_transfers::Column::StatsProcessed.eq(0i16))
                .all(tx)
                .await?;
            for t in &deferred_rows {
                STATS_TRANSFERS_DEFERRED_TOTAL
                    .with_label_values(&[deferral_reason(t, indexed)])
                    .inc();
            }
        }
    }

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

    // Union-find bookkeeping for merges within this batch: a winner can itself
    // become a loser of a later merge, so every asset id resolved before the
    // remap below must be corrected through `resolve_merged` (transitively).
    let mut merged_away: HashMap<i64, i64> = HashMap::new();
    // De-duplicates contradiction guards 1 and 2 within this batch: guard 2
    // (inside `merge_assets`) and guard 1 (below, post-remap) can both
    // observe the same underlying event, and one event must produce one
    // signal — see the module docs / ADR-011.
    let mut reported_contradiction_pairs: HashSet<(i64, i64)> = HashSet::new();

    // Resolve each transfer's stats asset(s). A transfer whose endpoints cannot
    // be reconciled (corrupt token data, conflicting mapping, or a merge
    // refused on chain collision) is skipped rather than aborting the batch —
    // otherwise one bad transfer would roll back the shared maintenance
    // transaction (message + cursor writes) every cycle. Skipped countable
    // transfers are still marked processed below so they are not retried
    // forever; skipped already-counted (repair-path) transfers simply keep
    // their existing link untouched. A `conversion` transfer with only one
    // known endpoint is a *deferral*, not a skip: it must never be marked
    // processed, on either path.
    //
    // `identity_ready` holds for every row this query returned, so
    // `ensure_asset_for_transfer` runs unconditionally; only *counting*
    // (edge accumulation + `stats_processed`/`transfers_count`/
    // `cumulative_amount`) is gated on `stats_processed == 0`.
    let mut proj_transfers: Vec<crosschain_transfers::Model> = Vec::with_capacity(transfers.len());
    let mut asset_pairs: Vec<(i64, i64)> = Vec::with_capacity(transfers.len());
    let mut edge_key_per_transfer: Vec<EdgeKey> = Vec::with_capacity(transfers.len());
    let mut skipped_ids: Vec<i64> = Vec::new();
    let mut repair_transfers: Vec<crosschain_transfers::Model> = Vec::new();
    let mut repair_asset_pairs: Vec<(i64, i64)> = Vec::new();
    let mut repair_updates: Vec<(i64, i64, i64)> = Vec::new();
    for t in &transfers {
        let countable = t.stats_processed == 0;
        match ensure_asset_for_transfer(
            tx,
            t,
            &mut token_to_asset,
            &mut merged_away,
            &mut reported_contradiction_pairs,
        )
        .await?
        {
            AssetResolution::Resolved(src, dst) if countable => {
                edge_key_per_transfer.push((
                    src,
                    dst,
                    t.bridge_id,
                    t.token_src_chain_id,
                    t.token_dst_chain_id,
                ));
                asset_pairs.push((src, dst));
                proj_transfers.push(t.clone());
            }
            AssetResolution::Resolved(src, dst) => {
                // Repair path: identity maintenance only. Never touches
                // `stats_processed`, `transfers_count`, or `cumulative_amount`.
                repair_asset_pairs.push((src, dst));
                repair_transfers.push(t.clone());
                repair_updates.push((t.id, src, dst));
            }
            AssetResolution::Conflict if countable => skipped_ids.push(t.id),
            // Repair-path refusal: keep the existing link, do nothing further.
            // `merge_assets` already logged/metric-recorded the refusal.
            AssetResolution::Conflict => {}
            // Deferred either way: leave the row exactly as it is. It is
            // re-evaluated the next time this transfer's canonical key is
            // flushed.
            AssetResolution::Deferred => {}
        }
    }

    // A merge inside the loop above can invalidate asset ids resolved for
    // *earlier* transfers in this same batch (work item 4): remap every
    // collected asset id through the transitive `merged_away` chain before
    // any of them is used again, so no dangling asset id is ever read or
    // written from here on. Both components of every pair are remapped
    // independently.
    for pair in asset_pairs.iter_mut() {
        pair.0 = resolve_merged(&merged_away, pair.0);
        pair.1 = resolve_merged(&merged_away, pair.1);
    }
    for key in edge_key_per_transfer.iter_mut() {
        key.0 = resolve_merged(&merged_away, key.0);
        key.1 = resolve_merged(&merged_away, key.1);
    }
    for pair in repair_asset_pairs.iter_mut() {
        pair.0 = resolve_merged(&merged_away, pair.0);
        pair.1 = resolve_merged(&merged_away, pair.1);
    }
    for (_, src, dst) in repair_updates.iter_mut() {
        *src = resolve_merged(&merged_away, *src);
        *dst = resolve_merged(&merged_away, *dst);
    }
    // The de-duplication set is remapped for the same reason, and it is easy to
    // forget: guard 2 records a pair using the ids that were current when *its*
    // merge ran, but a later merge in the same batch can move both of them on.
    // Comparing a post-remap pair against pre-remap keys would report one
    // underlying contradiction twice, under two different labels.
    reported_contradiction_pairs = reported_contradiction_pairs
        .into_iter()
        .map(|(src, dst)| {
            (
                resolve_merged(&merged_away, src),
                resolve_merged(&merged_away, dst),
            )
        })
        .collect();

    // Contradiction guard 1, run post-remap (see the module docs / ADR-011 for
    // why it cannot run inline in `ensure_conversion_asset_for_transfer`): a
    // `conversion` transfer whose two endpoints now resolve to the *same*
    // asset. Covers both the counted and repair sets, de-duplicated against
    // guard 2 (and against itself) by `reported_contradiction_pairs`.
    for (t, pair) in proj_transfers
        .iter()
        .zip(&asset_pairs)
        .chain(repair_transfers.iter().zip(&repair_asset_pairs))
    {
        if t.asset_linkage == Some(TransferAssetLinkage::Conversion)
            && pair.0 == pair.1
            && reported_contradiction_pairs.insert(*pair)
        {
            tracing::warn!(
                transfer_id = t.id,
                bridge_id = t.bridge_id,
                stats_asset_id = pair.0,
                "stats projection: conversion transfer resolves to one asset; an earlier \
                 mirror declaration or bad token data already merged them"
            );
            STATS_ASSET_LINKAGE_CONTRADICTION_TOTAL
                .with_label_values(&["conversion_self_asset"])
                .inc();
        }
    }

    let existing_edges = load_stats_asset_edges_for_keys(tx, &edge_key_per_transfer).await?;
    let mut edge_acc: HashMap<EdgeKey, EdgeAccum> = HashMap::new();
    let mut decimals_conflict_ids: Vec<i64> = Vec::new();
    // Paired with `decimals_conflict_ids`: unlike a mapping conflict, a
    // decimals conflict fires *after* `ensure_asset_for_transfer` already
    // resolved this transfer's asset(s) unambiguously — only the amount could
    // not be safely counted. Carry the resolved pair forward so it can still
    // be linked (see the write-up below).
    let mut decimals_conflict_asset_pairs: Vec<(i64, i64, i64)> = Vec::new();
    // A `conversion` transfer whose edge-side amount is absent (work item
    // 10i). Deferred exactly like `conversion_endpoint_unresolved`: never
    // marked processed, never linked, never contributes to the edge.
    let mut deferred_amount_ids: HashSet<i64> = HashSet::new();

    for (t, &(src_asset, dst_asset)) in proj_transfers.iter().zip(&asset_pairs) {
        let edge_key: EdgeKey = (
            src_asset,
            dst_asset,
            t.bridge_id,
            t.token_src_chain_id,
            t.token_dst_chain_id,
        );
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

        // Determine this edge's amount_side without mutating `edge_acc` yet:
        // either the side already fixed by an existing DB row, the side a
        // sibling transfer in this batch already picked for a brand-new edge,
        // or (first transfer to touch a brand-new edge) the side a fresh edge
        // would use.
        let amount_side = match existing_edges.get(&edge_key) {
            Some(edge) => edge.amount_side.clone(),
            None => match edge_acc.get(&edge_key) {
                Some(acc) => acc.amount_side().clone(),
                None => {
                    if source_chain_indexed || src_dec.is_some() {
                        EdgeAmountSide::Source
                    } else {
                        EdgeAmountSide::Destination
                    }
                }
            },
        };

        // Conversion amounts never fall back across sides (work item 10i): if
        // this transfer's chosen-side amount is absent, defer it entirely
        // rather than write a wrong asset's quantity onto this edge.
        if t.asset_linkage == Some(TransferAssetLinkage::Conversion)
            && transfer_amount_for_side(t, &amount_side).is_none()
        {
            deferred_amount_ids.insert(t.id);
            STATS_TRANSFERS_DEFERRED_TOTAL
                .with_label_values(&["amount_side_missing"])
                .inc();
            continue;
        }

        // A decimals conflict (task Decision 7) skips only this transfer — it
        // never aborts the batch. Every accumulator mutation happens strictly
        // after the conflict check inside `apply_transfer`, so a `Err` here
        // leaves the accumulator (new or already in the map) exactly as it was
        // before this transfer.
        let outcome: Result<(), DecimalsConflict> = match edge_acc.entry(edge_key) {
            Entry::Vacant(v) => {
                if let Some(edge) = existing_edges.get(&edge_key) {
                    let mut acc = EdgeAccum::FromDb {
                        db_decimals: edge.decimals,
                        working_decimals: edge.decimals,
                        amount_side: edge.amount_side.clone(),
                        delta_count: 0,
                        delta_amount: BigDecimal::from(0u64),
                    };
                    match acc.apply_transfer(src_asset, dst_asset, t, src_dec, dst_dec) {
                        Ok(()) => {
                            v.insert(acc);
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    let decimals = match amount_side {
                        EdgeAmountSide::Source => src_dec,
                        EdgeAmountSide::Destination => dst_dec,
                    };
                    let cumulative = transfer_amount_for_side(t, &amount_side)
                        .unwrap_or_else(|| BigDecimal::from(0u64));
                    v.insert(EdgeAccum::NewInBatch {
                        amount_side,
                        working_decimals: decimals,
                        count: 1,
                        cumulative,
                    });
                    Ok(())
                }
            }
            Entry::Occupied(mut o) => o
                .get_mut()
                .apply_transfer(src_asset, dst_asset, t, src_dec, dst_dec),
        };

        if outcome.is_err() {
            decimals_conflict_ids.push(t.id);
            decimals_conflict_asset_pairs.push((t.id, src_asset, dst_asset));
        }
    }

    // Remove decimals-conflict and amount-deferred transfers from the counted
    // set. Decimals-conflict transfers are skipped (marked processed, edge not
    // incremented) but their asset link is still written below — identity was
    // already resolved unambiguously; only the amount could not be safely
    // folded into the edge aggregate. Amount-deferred transfers are removed
    // entirely: no link, no processed marker, no edge contribution — the same
    // treatment as `conversion_endpoint_unresolved`.
    let decimals_conflict_set: HashSet<i64> = decimals_conflict_ids.into_iter().collect();
    let decimals_conflict_count = decimals_conflict_set.len();
    let (proj_transfers, asset_pairs): (Vec<_>, Vec<_>) =
        if decimals_conflict_set.is_empty() && deferred_amount_ids.is_empty() {
            (proj_transfers, asset_pairs)
        } else {
            proj_transfers
                .into_iter()
                .zip(asset_pairs)
                .filter(|(t, _)| {
                    !decimals_conflict_set.contains(&t.id) && !deferred_amount_ids.contains(&t.id)
                })
                .unzip()
        };

    for (key, accum) in edge_acc {
        match accum {
            EdgeAccum::FromDb {
                db_decimals,
                working_decimals,
                delta_count,
                delta_amount,
                ..
            } => {
                let (src_stats_asset_id, dst_stats_asset_id, bridge_id, src_chain_id, dst_chain_id) =
                    key;
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
                    .filter(stats_asset_edges::Column::SrcStatsAssetId.eq(src_stats_asset_id))
                    .filter(stats_asset_edges::Column::DstStatsAssetId.eq(dst_stats_asset_id))
                    .filter(stats_asset_edges::Column::BridgeId.eq(bridge_id))
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
                let (src_stats_asset_id, dst_stats_asset_id, bridge_id, src_chain_id, dst_chain_id) =
                    key;
                stats_asset_edges::Entity::insert(stats_asset_edges::ActiveModel {
                    src_stats_asset_id: Set(src_stats_asset_id),
                    dst_stats_asset_id: Set(dst_stats_asset_id),
                    bridge_id: Set(bridge_id),
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

    // Enrichment covers both newly counted and repair-path assets: a
    // previously-unknown token endpoint linked during repair may carry
    // metadata that fills a still-empty `stats_assets` field.
    let mut enrich_transfers = proj_transfers.clone();
    enrich_transfers.extend(repair_transfers.iter().cloned());
    let mut enrich_asset_pairs = asset_pairs.clone();
    enrich_asset_pairs.extend(repair_asset_pairs.iter().copied());
    enrich_stats_assets_for_batch(tx, &enrich_transfers, &enrich_asset_pairs, &token_rows).await?;

    let mut by_pair: HashMap<(i64, i64), Vec<i64>> = HashMap::new();
    for (t, &pair) in proj_transfers.iter().zip(&asset_pairs) {
        by_pair.entry(pair).or_default().push(t.id);
    }
    for ((src, dst), ids) in by_pair {
        run_in_batches(&ids, 1, |batch| async {
            crosschain_transfers::Entity::update_many()
                .col_expr(
                    crosschain_transfers::Column::SrcStatsAssetId,
                    Expr::value(src),
                )
                .col_expr(
                    crosschain_transfers::Column::DstStatsAssetId,
                    Expr::value(dst),
                )
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

    // Mark mapping-conflict-skipped transfers processed, with no stats asset
    // link: the mapping conflict means asset identity is genuinely unknown or
    // ambiguous for this transfer (its endpoints could not be reconciled), so
    // there is no id to write. Both asset columns `IS NULL` after this update
    // means exactly that — identity unknown.
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

    // Mark decimals-conflict-skipped transfers processed *and* link the
    // resolved asset pair. ADR-004 Decision 3 separates counting from
    // identity: here counting failed (the edge amount could not be safely
    // folded in, so `transfers_count`/`cumulative_amount` are untouched and
    // this batch's `stats_asset_edges` mutation above never saw this
    // transfer) but identity succeeded — `ensure_asset_for_transfer` resolved
    // this exact transfer's pair unambiguously before the conflict fired.
    // Discarding that identity would conflate the two concerns again and
    // collapse this case with a genuine mapping conflict. After this update,
    // both asset columns set together with `stats_processed > 0` and no edge
    // contribution reads unambiguously as "identity known, amount not
    // counted" — distinct from both columns `IS NULL`, which stays reserved
    // for "identity unknown" (the mapping-conflict case above).
    //
    // This is inert for counting: `stats_processed` is already non-zero after
    // this same update (it is set together with the link, gated by the same
    // `StatsProcessed.eq(0i16)` filter used everywhere else in this
    // function), so the repair path's `StatsProcessed.gt(0i16)` filter is the
    // only thing that can ever touch these columns for this row again, and it
    // never touches `stats_processed`/`transfers_count`/`cumulative_amount`.
    // Nothing can recount this transfer either way.
    if !decimals_conflict_asset_pairs.is_empty() {
        let mut by_pair_decimals_conflict: HashMap<(i64, i64), Vec<i64>> = HashMap::new();
        for (tid, src, dst) in decimals_conflict_asset_pairs {
            by_pair_decimals_conflict
                .entry((src, dst))
                .or_default()
                .push(tid);
        }
        for ((src, dst), ids) in by_pair_decimals_conflict {
            run_in_batches(&ids, 1, |batch| async {
                crosschain_transfers::Entity::update_many()
                    .col_expr(
                        crosschain_transfers::Column::SrcStatsAssetId,
                        Expr::value(src),
                    )
                    .col_expr(
                        crosschain_transfers::Column::DstStatsAssetId,
                        Expr::value(dst),
                    )
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
    }

    // Repair path: link the (possibly merged) asset pair for an already-counted
    // transfer. Idempotent, and deliberately excludes `stats_processed`,
    // `transfers_count`, and `cumulative_amount` — see the function docs.
    if !repair_updates.is_empty() {
        let mut by_pair_repair: HashMap<(i64, i64), Vec<i64>> = HashMap::new();
        for (tid, src, dst) in repair_updates {
            by_pair_repair.entry((src, dst)).or_default().push(tid);
        }
        for ((src, dst), ids) in by_pair_repair {
            run_in_batches(&ids, 1, |batch| async {
                crosschain_transfers::Entity::update_many()
                    .col_expr(
                        crosschain_transfers::Column::SrcStatsAssetId,
                        Expr::value(src),
                    )
                    .col_expr(
                        crosschain_transfers::Column::DstStatsAssetId,
                        Expr::value(dst),
                    )
                    .col_expr(
                        crosschain_transfers::Column::UpdatedAt,
                        Expr::current_timestamp().into(),
                    )
                    .filter(crosschain_transfers::Column::Id.is_in(batch.iter().copied()))
                    .filter(crosschain_transfers::Column::StatsProcessed.gt(0i16))
                    .exec(tx)
                    .await?;
                Ok(())
            })
            .await?;
        }
    }

    Ok(proj_transfers.len() + skipped_ids.len() + decimals_conflict_count)
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
            src_stats_asset_id: None,
            dst_stats_asset_id: None,
            asset_linkage: None,
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

// Isolated coverage for `deferral_reason`, the pure classifier behind the
// `STATS_TRANSFERS_DEFERRED_TOTAL` metric label (work item 10a). Per
// `.memory-bank/rules/testing.md`, metric emission must be covered by a unit
// test on the pure decision function rather than a before/after delta on the
// shared `lazy_static` counter — no database, no shared process state, so
// this cannot race with any other test.
#[cfg(test)]
mod deferral_reason_tests {
    use super::deferral_reason;
    use interchain_indexer_entity::{
        crosschain_transfers, sea_orm_active_enums::TransferAssetLinkage,
    };
    use sea_orm::prelude::BigDecimal;

    use crate::stats::indexed_chains::IndexedChains;

    fn transfer(
        asset_linkage: Option<TransferAssetLinkage>,
        token_src_address: Option<Vec<u8>>,
        token_dst_address: Option<Vec<u8>>,
    ) -> crosschain_transfers::Model {
        crosschain_transfers::Model {
            id: 1,
            message_id: 1,
            bridge_id: 1,
            index: 0,
            r#type: None,
            token_src_chain_id: 1,
            token_dst_chain_id: 100,
            src_amount: Some(BigDecimal::from(0u64)),
            dst_amount: Some(BigDecimal::from(0u64)),
            token_src_address,
            token_dst_address,
            sender_address: None,
            recipient_address: None,
            token_ids: None,
            stats_processed: 0,
            src_stats_asset_id: None,
            dst_stats_asset_id: None,
            asset_linkage,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn unknown_linkage_is_reported_first_even_with_a_missing_endpoint() {
        let t = transfer(None, None, Some(vec![1]));
        assert_eq!(
            deferral_reason(&t, &IndexedChains::AllIndexed),
            "linkage_unknown",
            "an unstated linkage cannot be resolved by the endpoint arriving, so it \
             must take precedence over identity_incomplete"
        );
    }

    #[test]
    fn missing_endpoint_on_an_indexed_chain_is_identity_incomplete() {
        let t = transfer(Some(TransferAssetLinkage::Mirror), None, Some(vec![1]));
        assert_eq!(
            deferral_reason(&t, &IndexedChains::AllIndexed),
            "identity_incomplete"
        );
    }

    #[test]
    fn both_endpoints_known_falls_back_to_awaiting_confirmation() {
        let t = transfer(
            Some(TransferAssetLinkage::Mirror),
            Some(vec![1]),
            Some(vec![2]),
        );
        assert_eq!(
            deferral_reason(&t, &IndexedChains::AllIndexed),
            "awaiting_confirmation"
        );
    }

    #[test]
    fn missing_endpoint_on_an_unindexed_chain_is_not_identity_incomplete() {
        let indexed = IndexedChains::from_pairs([(1, 1)]); // bridge 1 indexes chain 1 only
        let t = transfer(Some(TransferAssetLinkage::Mirror), Some(vec![1]), None);
        // token_dst_chain_id (100) is unindexed for bridge 1, so the missing
        // destination endpoint can never arrive -- not an identity gap.
        assert_eq!(deferral_reason(&t, &indexed), "awaiting_confirmation");
    }
}

// Isolated coverage for the decision behind `STATS_EDGE_DECIMALS_CONFLICT_TOTAL`
// (see `warn_edge_decimals_mismatch`). This deliberately does NOT read the
// metric itself: `STATS_EDGE_DECIMALS_CONFLICT_TOTAL` is a process-wide
// `lazy_static` counter shared by every test in this binary, so a
// before/after delta on it is not test-isolated under `cargo test`'s default
// parallelism — that pattern is what made
// `database::tests::test_decimals_conflict_on_counting_path_skips_transfer_without_aborting`
// flaky (see the comment there). Instead, this tests the pure decision
// function directly: a decimals mismatch must return `Err(DecimalsConflict)`
// (the trigger for both the metric increment and the caller's
// skip-not-abort behaviour), while a match or a not-yet-known decimals value
// must resolve the amount without conflict. No shared state is touched, so
// this cannot race with any other test, present or future.
#[cfg(test)]
mod edge_transfer_amount_for_side_tests {
    use super::{DecimalsConflict, edge_transfer_amount_for_side, transfer_amount_for_side};
    use interchain_indexer_entity::{crosschain_transfers, sea_orm_active_enums::EdgeAmountSide};
    use sea_orm::prelude::BigDecimal;

    fn transfer(src_amount: Option<u64>, dst_amount: Option<u64>) -> crosschain_transfers::Model {
        crosschain_transfers::Model {
            id: 1,
            message_id: 1,
            bridge_id: 1,
            index: 0,
            r#type: None,
            token_src_chain_id: 1,
            token_dst_chain_id: 100,
            src_amount: src_amount.map(BigDecimal::from),
            dst_amount: dst_amount.map(BigDecimal::from),
            token_src_address: None,
            token_dst_address: None,
            sender_address: None,
            recipient_address: None,
            token_ids: None,
            stats_processed: 0,
            src_stats_asset_id: None,
            dst_stats_asset_id: None,
            asset_linkage: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn matching_decimals_resolves_amount_without_conflict() {
        let t = transfer(Some(10), Some(20));
        let mut working_decimals = Some(18i16);
        let amount = edge_transfer_amount_for_side(
            &EdgeAmountSide::Source,
            &mut working_decimals,
            &t,
            Some(18),
            Some(6),
            42,
            43,
        )
        .expect("matching decimals must not conflict");
        assert_eq!(amount, BigDecimal::from(10u64));
        assert_eq!(
            working_decimals,
            Some(18),
            "the already-known decimals are unchanged"
        );
    }

    #[test]
    fn mismatched_decimals_triggers_the_conflict_marker() {
        let t = transfer(Some(10), Some(20));
        let mut working_decimals = Some(18i16);
        let err = edge_transfer_amount_for_side(
            &EdgeAmountSide::Source,
            &mut working_decimals,
            &t,
            Some(17), // disagrees with working_decimals (18)
            Some(6),
            42,
            43,
        )
        .expect_err("a decimals mismatch must be reported as a conflict, not silently accepted");
        // `DecimalsConflict` carries no fields; reaching this arm at all is
        // the behaviour under test — it is exactly the trigger that makes the
        // caller invoke `warn_edge_decimals_mismatch` (which increments
        // `STATS_EDGE_DECIMALS_CONFLICT_TOTAL`).
        match err {
            DecimalsConflict => {}
        }
    }

    #[test]
    fn unknown_working_decimals_adopts_incoming_without_conflict() {
        let t = transfer(Some(10), Some(20));
        let mut working_decimals = None;
        let amount = edge_transfer_amount_for_side(
            &EdgeAmountSide::Source,
            &mut working_decimals,
            &t,
            Some(9),
            Some(6),
            42,
            43,
        )
        .expect("no prior decimals means nothing to conflict with");
        assert_eq!(amount, BigDecimal::from(10u64));
        assert_eq!(
            working_decimals,
            Some(9),
            "the first-seen decimals are adopted"
        );
    }

    #[test]
    fn missing_primary_amount_falls_back_to_the_opposite_side() {
        let t = transfer(None, Some(20));
        assert_eq!(
            transfer_amount_for_side(&t, &EdgeAmountSide::Source),
            Some(BigDecimal::from(20u64)),
            "a mirror destination-only transfer falls back to dst_amount for the source side"
        );
    }

    #[test]
    fn conversion_never_falls_back_across_sides() {
        let mut t = transfer(None, Some(20));
        t.asset_linkage =
            Some(interchain_indexer_entity::sea_orm_active_enums::TransferAssetLinkage::Conversion);
        assert_eq!(
            transfer_amount_for_side(&t, &EdgeAmountSide::Source),
            None,
            "a conversion transfer must defer rather than borrow the other asset's amount"
        );
        assert_eq!(
            transfer_amount_for_side(&t, &EdgeAmountSide::Destination),
            Some(BigDecimal::from(20u64)),
            "the side that does have an amount still resolves normally"
        );
    }
}
