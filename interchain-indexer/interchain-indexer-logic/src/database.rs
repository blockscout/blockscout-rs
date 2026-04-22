use chrono::{Duration, NaiveDate, NaiveDateTime};
use interchain_indexer_entity::{
    avalanche_icm_blockchain_ids, bridge_contracts, bridges, chains, crosschain_messages,
    crosschain_transfers, indexer_checkpoints, pending_messages,
    sea_orm_active_enums::{EdgeAmountSide, MessageStatus, TransferType},
    stats_asset_edges, stats_asset_tokens, stats_assets, stats_chains, stats_messages, tokens,
};
use parking_lot::RwLock;
use sea_orm::{
    ActiveValue, ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DbErr, EntityTrait, FromQueryResult, JoinType, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, RelationTrait, Statement, StatementBuilder, TransactionTrait, Value,
    entity::prelude::*,
    prelude::Expr,
    sea_query::{Alias, Asterisk, Func, OnConflict, Query, SelectStatement, UnionType},
};
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use crate::{
    TokenInfoService,
    pagination::{
        MessagesPaginationLogic, OutputPagination, PaginationDirection, TransfersPaginationLogic,
    },
};

pub struct InterchainTotalCounters {
    pub timestamp: NaiveDateTime,
    pub total_messages: u64,
    pub total_transfers: u64,
}

pub struct InterchainDailyCounters {
    pub date: NaiveDate,
    pub daily_messages: u64,
    pub daily_transfers: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, FromQueryResult)]
pub struct MessagePathStatsRow {
    pub src_chain_id: i64,
    pub dst_chain_id: i64,
    pub messages_count: i64,
}

#[derive(Debug, FromQueryResult)]
pub struct JoinedTransfer {
    // transfer fields
    pub id: i64,
    pub message_id: i64,
    pub bridge_id: i32,
    pub index: i16,
    pub r#type: Option<TransferType>,
    pub token_src_chain_id: i64,
    pub token_dst_chain_id: i64,
    pub src_amount: BigDecimal,
    pub dst_amount: BigDecimal,
    pub token_src_address: Vec<u8>,
    pub token_dst_address: Vec<u8>,
    pub sender_address: Option<Vec<u8>>,
    pub recipient_address: Option<Vec<u8>>,
    pub token_ids: Option<Vec<Decimal>>,

    // joined message fields
    pub status: MessageStatus,
    pub init_timestamp: NaiveDateTime,
    pub last_update_timestamp: Option<NaiveDateTime>,
    pub native_id: Option<Vec<u8>>,
    pub src_tx_hash: Option<Vec<u8>>,
    pub dst_tx_hash: Option<Vec<u8>>,
}

/// Batch size for startup stats backfill (per message pass and per transfer pass each round).
pub const STATS_BACKFILL_BATCH: u64 = 50;

#[derive(Debug, Default, Clone)]
pub struct BackfillStatsReport {
    pub messages_processed: usize,
    pub transfers_processed: usize,
    /// Src/dst token keys from transfers projected this round (kickoff enrichment **after** tx).
    pub token_keys_for_enrichment: Vec<(i64, Vec<u8>)>,
}

#[derive(Clone)]
pub struct InterchainDatabase {
    pub db: Arc<DatabaseConnection>,

    bridges_names: Arc<RwLock<HashMap<i32, String>>>, // Lazy loaded bridge names
}

/// Per-chain count of distinct `(chain_id, user_address)` from `crosschain_messages` (sender ∪ recipient).
fn select_stats_chains_message_user_counts() -> SelectStatement {
    let pairs = Query::select()
        .expr_as(
            Expr::col(crosschain_messages::Column::SrcChainId),
            Alias::new("chain_id"),
        )
        .expr_as(
            Expr::col(crosschain_messages::Column::SenderAddress),
            Alias::new("addr"),
        )
        .from(crosschain_messages::Entity)
        .and_where(Expr::col(crosschain_messages::Column::SenderAddress).is_not_null())
        .union(
            UnionType::Distinct,
            Query::select()
                .expr_as(
                    Expr::col(crosschain_messages::Column::DstChainId),
                    Alias::new("chain_id"),
                )
                .expr_as(
                    Expr::col(crosschain_messages::Column::RecipientAddress),
                    Alias::new("addr"),
                )
                .from(crosschain_messages::Entity)
                .and_where(Expr::col(crosschain_messages::Column::DstChainId).is_not_null())
                .and_where(Expr::col(crosschain_messages::Column::RecipientAddress).is_not_null())
                .take(),
        )
        .take();

    Query::select()
        .column(Alias::new("chain_id"))
        .expr_as(Func::count(Expr::col(Asterisk)), Alias::new("user_count"))
        .from_subquery(pairs, Alias::new("u"))
        .group_by_col(Alias::new("chain_id"))
        .take()
}

/// Per-chain count of distinct `(chain_id, user_address)` from `crosschain_transfers` (sender ∪ recipient).
fn select_stats_chains_transfer_user_counts() -> SelectStatement {
    let pairs = Query::select()
        .expr_as(
            Expr::col(crosschain_transfers::Column::TokenSrcChainId),
            Alias::new("chain_id"),
        )
        .expr_as(
            Expr::col(crosschain_transfers::Column::SenderAddress),
            Alias::new("addr"),
        )
        .from(crosschain_transfers::Entity)
        .and_where(Expr::col(crosschain_transfers::Column::SenderAddress).is_not_null())
        .union(
            UnionType::Distinct,
            Query::select()
                .expr_as(
                    Expr::col(crosschain_transfers::Column::TokenDstChainId),
                    Alias::new("chain_id"),
                )
                .expr_as(
                    Expr::col(crosschain_transfers::Column::RecipientAddress),
                    Alias::new("addr"),
                )
                .from(crosschain_transfers::Entity)
                .and_where(Expr::col(crosschain_transfers::Column::RecipientAddress).is_not_null())
                .take(),
        )
        .take();

    Query::select()
        .column(Alias::new("chain_id"))
        .expr_as(Func::count(Expr::col(Asterisk)), Alias::new("user_count"))
        .from_subquery(pairs, Alias::new("u"))
        .group_by_col(Alias::new("chain_id"))
        .take()
}

#[derive(Copy, Clone)]
enum MessagePathDirection {
    Outgoing,
    Incoming,
}

fn should_expand_message_paths(
    include_zero_chains: bool,
    counterparty_chain_ids: Option<&[i64]>,
) -> bool {
    include_zero_chains
        && counterparty_chain_ids
            .map(|chain_ids| chain_ids.is_empty())
            .unwrap_or(true)
}

fn build_all_time_message_paths_query(
    chain_id: i64,
    direction: MessagePathDirection,
    counterparty_chain_ids: Option<&[i64]>,
    include_zero_chains: bool,
) -> (String, Vec<Value>) {
    if should_expand_message_paths(include_zero_chains, counterparty_chain_ids) {
        let sql = match direction {
            MessagePathDirection::Outgoing => {
                r#"
SELECT $1::bigint AS src_chain_id,
       c.id AS dst_chain_id,
       COALESCE(sm.messages_count, 0)::bigint AS messages_count
FROM chains c
LEFT JOIN stats_messages sm
    ON sm.src_chain_id = $1
   AND sm.dst_chain_id = c.id
WHERE c.id <> $1
ORDER BY messages_count DESC, src_chain_id ASC, dst_chain_id ASC
"#
            }
            MessagePathDirection::Incoming => {
                r#"
SELECT c.id AS src_chain_id,
       $1::bigint AS dst_chain_id,
       COALESCE(sm.messages_count, 0)::bigint AS messages_count
FROM chains c
LEFT JOIN stats_messages sm
    ON sm.src_chain_id = c.id
   AND sm.dst_chain_id = $1
WHERE c.id <> $1
ORDER BY messages_count DESC, src_chain_id ASC, dst_chain_id ASC
"#
            }
        };

        return (sql.to_string(), vec![Value::BigInt(Some(chain_id))]);
    }

    let filter_column = match direction {
        MessagePathDirection::Outgoing => "src_chain_id",
        MessagePathDirection::Incoming => "dst_chain_id",
    };
    let counterparty_column = match direction {
        MessagePathDirection::Outgoing => "dst_chain_id",
        MessagePathDirection::Incoming => "src_chain_id",
    };

    let mut values = vec![Value::BigInt(Some(chain_id))];
    let mut sql = format!(
        r#"
SELECT src_chain_id, dst_chain_id, messages_count
FROM stats_messages
WHERE {filter_column} = $1"#
    );

    if let Some(ids) = counterparty_chain_ids.filter(|c| !c.is_empty()) {
        let placeholders: Vec<String> = (0..ids.len()).map(|i| format!("${}", i + 2)).collect();
        sql.push_str(&format!(
            " AND {counterparty_column} IN ({})",
            placeholders.join(", ")
        ));
        for &id in ids {
            values.push(Value::BigInt(Some(id)));
        }
    }

    sql.push_str("\nORDER BY messages_count DESC, src_chain_id ASC, dst_chain_id ASC\n");

    (sql, values)
}

fn build_bounded_message_paths_query(
    chain_id: i64,
    from_date: Option<NaiveDate>,
    to_date: Option<NaiveDate>,
    direction: MessagePathDirection,
    counterparty_chain_ids: Option<&[i64]>,
    include_zero_chains: bool,
) -> (String, Vec<Value>) {
    if should_expand_message_paths(include_zero_chains, counterparty_chain_ids) {
        let mut aggregate_where_parts = vec![match direction {
            MessagePathDirection::Outgoing => "src_chain_id = $1".to_string(),
            MessagePathDirection::Incoming => "dst_chain_id = $1".to_string(),
        }];
        let mut values = vec![Value::BigInt(Some(chain_id))];
        let mut placeholder = 2;

        if let Some(from_date) = from_date {
            aggregate_where_parts.push(format!("date >= ${placeholder}"));
            values.push(Value::ChronoDate(Some(Box::new(from_date))));
            placeholder += 1;
        }

        if let Some(to_date) = to_date {
            aggregate_where_parts.push(format!("date < ${placeholder}"));
            values.push(Value::ChronoDate(Some(Box::new(to_date))));
        }

        let sql = match direction {
            MessagePathDirection::Outgoing => format!(
                r#"
SELECT $1::bigint AS src_chain_id,
       c.id AS dst_chain_id,
       COALESCE(sm.messages_count, 0)::bigint AS messages_count
FROM chains c
LEFT JOIN (
    SELECT dst_chain_id,
           SUM(messages_count)::bigint AS messages_count
    FROM stats_messages_days
    WHERE {}
    GROUP BY dst_chain_id
) sm ON sm.dst_chain_id = c.id
WHERE c.id <> $1
ORDER BY messages_count DESC, src_chain_id ASC, dst_chain_id ASC
"#,
                aggregate_where_parts.join(" AND ")
            ),
            MessagePathDirection::Incoming => format!(
                r#"
SELECT c.id AS src_chain_id,
       $1::bigint AS dst_chain_id,
       COALESCE(sm.messages_count, 0)::bigint AS messages_count
FROM chains c
LEFT JOIN (
    SELECT src_chain_id,
           SUM(messages_count)::bigint AS messages_count
    FROM stats_messages_days
    WHERE {}
    GROUP BY src_chain_id
) sm ON sm.src_chain_id = c.id
WHERE c.id <> $1
ORDER BY messages_count DESC, src_chain_id ASC, dst_chain_id ASC
"#,
                aggregate_where_parts.join(" AND ")
            ),
        };

        return (sql, values);
    }

    let filter_column = match direction {
        MessagePathDirection::Outgoing => "src_chain_id",
        MessagePathDirection::Incoming => "dst_chain_id",
    };
    let counterparty_column = match direction {
        MessagePathDirection::Outgoing => "dst_chain_id",
        MessagePathDirection::Incoming => "src_chain_id",
    };

    let mut where_parts = vec![format!("{filter_column} = $1")];
    let mut values = vec![Value::BigInt(Some(chain_id))];
    let mut placeholder = 2;

    if let Some(from_date) = from_date {
        where_parts.push(format!("date >= ${placeholder}"));
        values.push(Value::ChronoDate(Some(Box::new(from_date))));
        placeholder += 1;
    }

    if let Some(to_date) = to_date {
        where_parts.push(format!("date < ${placeholder}"));
        values.push(Value::ChronoDate(Some(Box::new(to_date))));
        placeholder += 1;
    }

    if let Some(ids) = counterparty_chain_ids.filter(|c| !c.is_empty()) {
        let placeholders: Vec<String> = (0..ids.len())
            .map(|i| format!("${}", placeholder + i))
            .collect();
        where_parts.push(format!(
            "{counterparty_column} IN ({})",
            placeholders.join(", ")
        ));
        for &id in ids {
            values.push(Value::BigInt(Some(id)));
        }
    }

    (
        format!(
            r#"
SELECT src_chain_id,
       dst_chain_id,
       SUM(messages_count)::bigint AS messages_count
FROM stats_messages_days
WHERE {}
GROUP BY src_chain_id, dst_chain_id
ORDER BY messages_count DESC, src_chain_id ASC, dst_chain_id ASC
"#,
            where_parts.join(" AND ")
        ),
        values,
    )
}

impl InterchainDatabase {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            bridges_names: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // CONFIGURATION TABLE: chains
    pub async fn upsert_chains(&self, chains: Vec<chains::ActiveModel>) -> anyhow::Result<()> {
        if chains.is_empty() {
            return Ok(());
        }

        match chains::Entity::insert_many(chains)
            .on_conflict(
                OnConflict::column(chains::Column::Id)
                    .update_columns([
                        chains::Column::Name,
                        chains::Column::Icon,
                        chains::Column::Explorer,
                        chains::Column::CustomRoutes,
                    ])
                    .value(chains::Column::UpdatedAt, Expr::current_timestamp())
                    .to_owned(),
            )
            .exec(self.db.as_ref())
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!(err =? e, "Failed to upsert chains");
                Err(e.into())
            }
        }
    }

    pub async fn get_all_chains(&self) -> anyhow::Result<Vec<chains::Model>> {
        match chains::Entity::find()
            .order_by_asc(chains::Column::Id)
            .all(self.db.as_ref())
            .await
        {
            Ok(result) => Ok(result),

            Err(e) => {
                tracing::error!(err =? e, "Failed to fetch all chains");
                Err(e.into())
            }
        }
    }

    pub async fn get_chain_by_id(&self, chain_id: u64) -> anyhow::Result<Option<chains::Model>> {
        match chains::Entity::find_by_id(chain_id as i64)
            .one(self.db.as_ref())
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => {
                tracing::error!(err =? e, chain_id, "Failed to fetch chain by id");
                Err(e.into())
            }
        }
    }

    pub async fn ensure_chain_exists(
        &self,
        chain_id: i64,
        preferred_name: Option<String>,
        icon: Option<String>,
    ) -> anyhow::Result<()> {
        if chains::Entity::find_by_id(chain_id)
            .one(self.db.as_ref())
            .await?
            .is_some()
        {
            return Ok(());
        }

        let try_insert = |name: String, icon: Option<String>| async move {
            let model = chains::ActiveModel {
                id: ActiveValue::Set(chain_id),
                name: ActiveValue::Set(name),
                icon: ActiveValue::Set(icon),
                ..Default::default()
            };

            chains::Entity::insert(model)
                .on_conflict(
                    OnConflict::column(chains::Column::Id)
                        .do_nothing()
                        .to_owned(),
                )
                .exec(self.db.as_ref())
                .await
        };

        let name = preferred_name
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("EVM Chain {chain_id}"));

        match try_insert(name.clone(), icon.clone()).await {
            Ok(_) => Ok(()),
            Err(err) => {
                // Most commonly: UNIQUE(name) violation. Retry with a deterministic unique-ish name.
                tracing::warn!(
                    err = ?err,
                    chain_id,
                    name = %name,
                    "failed to insert chains row; retrying with fallback name"
                );

                let fallback = format!("EVM Chain {chain_id}");
                try_insert(fallback, icon).await?;
                Ok(())
            }
        }
    }

    /// Load a map of Avalanche blockchain IDs (bytes in `avalanche_icm_blockchain_ids`)
    /// normalized as `0x`-prefixed hex strings to chain id.
    ///
    /// This is used to pre-populate a per-indexer in-memory cache so handlers don't need to
    /// hit the database for every log.
    pub async fn load_native_id_map(&self) -> anyhow::Result<HashMap<String, i64>> {
        avalanche_icm_blockchain_ids::Entity::find()
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|row| {
                        (
                            format!("0x{}", hex::encode(row.blockchain_id)),
                            row.chain_id,
                        )
                    })
                    .collect::<HashMap<_, _>>()
            })
            .map_err(|e| {
                tracing::error!(
                    err = ?e,
                    "Failed to load avalanche_icm blockchain id -> chain id map"
                );
                e.into()
            })
    }

    pub async fn get_avalanche_icm_chain_id_by_blockchain_id(
        &self,
        blockchain_id: &[u8],
    ) -> anyhow::Result<Option<i64>> {
        let row = avalanche_icm_blockchain_ids::Entity::find_by_id(blockchain_id.to_vec())
            .one(self.db.as_ref())
            .await?;
        Ok(row.map(|m| m.chain_id))
    }

    pub async fn upsert_avalanche_icm_blockchain_id(
        &self,
        blockchain_id: Vec<u8>,
        chain_id: i64,
    ) -> anyhow::Result<()> {
        let insert = avalanche_icm_blockchain_ids::ActiveModel {
            blockchain_id: ActiveValue::Set(blockchain_id.clone()),
            chain_id: ActiveValue::Set(chain_id),
            ..Default::default()
        };

        // First, handle the common path: upsert by primary key (blockchain_id).
        // This covers re-mapping a previously seen blockchain_id.
        match avalanche_icm_blockchain_ids::Entity::insert(insert)
            .on_conflict(
                OnConflict::column(avalanche_icm_blockchain_ids::Column::BlockchainId)
                    .update_columns([avalanche_icm_blockchain_ids::Column::ChainId])
                    .value(
                        avalanche_icm_blockchain_ids::Column::UpdatedAt,
                        Expr::current_timestamp(),
                    )
                    .to_owned(),
            )
            .exec(self.db.as_ref())
            .await
        {
            Ok(_) => return Ok(()),
            Err(e) => {
                // If we hit UNIQUE(chain_id), update that row to point at the new blockchain_id.
                // (The mapping is conceptually 1 chain_id -> 1 blockchain_id.)
                let msg = e.to_string();
                let looks_like_unique_chain_id = msg.contains("avalanche_icm_blockchain_ids")
                    && msg.contains("chain_id")
                    && (msg.contains("duplicate") || msg.contains("unique"));
                if !looks_like_unique_chain_id {
                    return Err(e.into());
                }
            }
        }

        let res = avalanche_icm_blockchain_ids::Entity::update_many()
            .col_expr(
                avalanche_icm_blockchain_ids::Column::BlockchainId,
                Expr::val(blockchain_id).into(),
            )
            .col_expr(
                avalanche_icm_blockchain_ids::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(avalanche_icm_blockchain_ids::Column::ChainId.eq(chain_id))
            .exec(self.db.as_ref())
            .await?;

        if res.rows_affected == 0 {
            return Err(anyhow::anyhow!(
                "failed to upsert avalanche_icm_blockchain_ids: insert \
                 failed and no row updated"
            ));
        }

        Ok(())
    }

    // CONFIGURATION TABLE: bridges
    // Updating the name of a bridge with an existing ID is prohibited
    // Renaming a bridge is allowed only via a direct SQL request
    pub async fn upsert_bridges(&self, bridges: Vec<bridges::ActiveModel>) -> anyhow::Result<()> {
        // Extract id and name from input bridges for validation
        let bridge_id_name_map: HashMap<i32, String> = bridges
            .iter()
            .filter_map(|bridge| match (&bridge.id, &bridge.name) {
                (ActiveValue::Set(id), ActiveValue::Set(name)) => Some((*id, name.clone())),
                _ => None,
            })
            .collect();

        // Check existing bridges and validate id+name match
        let bridge_ids: Vec<i32> = bridge_id_name_map.keys().copied().collect();
        if !bridge_ids.is_empty() {
            match bridges::Entity::find()
                .filter(bridges::Column::Id.is_in(bridge_ids))
                .all(self.db.as_ref())
                .await
            {
                Ok(existing_bridges) => {
                    for existing in existing_bridges {
                        if let Some(expected_name) = bridge_id_name_map.get(&existing.id)
                            && existing.name != *expected_name
                        {
                            let err_msg = format!(
                                "Bridge with id {} exists but has different \
                                 name: expected '{}', found '{}'",
                                existing.id, expected_name, existing.name
                            );
                            tracing::error!("{}", err_msg);
                            return Err(anyhow::anyhow!(err_msg));
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(err =? e, "Failed to check existing bridges");
                    return Err(e.into());
                }
            }
        }

        self.db
            .transaction::<_, (), DbErr>(|tx| {
                Box::pin(async move {
                    // First, disable all existing bridges
                    // The upsert below will set the appropriate enabled flags for bridges in the input list
                    bridges::Entity::update_many()
                        .col_expr(bridges::Column::Enabled, Expr::value(false))
                        .exec(tx)
                        .await?;

                    // Next proceed with upsert (if any)
                    if !bridges.is_empty() {
                        bridges::Entity::insert_many(bridges)
                            .on_conflict(
                                OnConflict::column(bridges::Column::Id)
                                    .update_columns([
                                        bridges::Column::Type,
                                        bridges::Column::Enabled,
                                        bridges::Column::ApiUrl,
                                        bridges::Column::UiUrl,
                                        bridges::Column::DocsUrl,
                                    ])
                                    .to_owned(),
                            )
                            .exec(tx)
                            .await?;
                    }

                    Ok(())
                })
            })
            .await?;

        // Most likely bridges upserting will be invoked just on service startup,
        // but just in case, we invalidate the cache anyway.
        self.bridges_names.write().clear();

        Ok(())
    }

    pub async fn get_all_bridges(&self) -> anyhow::Result<Vec<bridges::Model>> {
        match bridges::Entity::find().all(self.db.as_ref()).await {
            Ok(result) => Ok(result),
            Err(e) => {
                tracing::error!(err =? e, "Failed to fetch all bridges");
                Err(e.into())
            }
        }
    }

    pub async fn get_bridge_name(&self, bridge_id: i32) -> anyhow::Result<String> {
        if self.bridges_names.read().is_empty() {
            let bridges = self.get_all_bridges().await?;
            *self.bridges_names.write() = bridges.into_iter().map(|b| (b.id, b.name)).collect();
        }

        self.bridges_names
            .read()
            .get(&bridge_id)
            .cloned()
            .ok_or(anyhow::anyhow!("Unknown bridge id: {}", bridge_id))
    }

    pub async fn get_bridge(&self, bridge_id: i32) -> anyhow::Result<Option<bridges::Model>> {
        match bridges::Entity::find()
            .filter(bridges::Column::Id.eq(bridge_id))
            .one(self.db.as_ref())
            .await
        {
            Ok(Some(result)) => Ok(Some(result)),
            Ok(None) => {
                tracing::error!(bridge_id =? bridge_id, "Bridge not found");
                Ok(None)
            }
            Err(e) => {
                tracing::error!(err =? e, bridge_id =? bridge_id, "Failed to fetch the bridge");
                Err(e.into())
            }
        }
    }

    // STATS ASSETS: canonical asset mapping and aggregated edges
    pub async fn create_stats_asset(
        &self,
        name: Option<String>,
        symbol: Option<String>,
        icon_url: Option<String>,
    ) -> anyhow::Result<stats_assets::Model> {
        let model = stats_assets::ActiveModel {
            name: ActiveValue::Set(name),
            symbol: ActiveValue::Set(symbol),
            icon_url: ActiveValue::Set(icon_url),
            ..Default::default()
        };
        match stats_assets::Entity::insert(model)
            .exec_with_returning(self.db.as_ref())
            .await
        {
            Ok(m) => Ok(m),
            Err(e) => {
                tracing::error!(err = ?e, "Failed to create stats asset");
                Err(e.into())
            }
        }
    }

    /// Links a token (chain_id, token_address) to a stats asset. Does not require a row in `tokens`.
    /// Fails if (chain_id, token_address) is already linked to another stats asset, or if
    /// this stats asset already has a different token on the same chain.
    pub async fn link_token_to_stats_asset(
        &self,
        stats_asset_id: i64,
        chain_id: i64,
        token_address: Vec<u8>,
    ) -> anyhow::Result<()> {
        let model = stats_asset_tokens::ActiveModel {
            stats_asset_id: ActiveValue::Set(stats_asset_id),
            chain_id: ActiveValue::Set(chain_id),
            token_address: ActiveValue::Set(token_address),
            ..Default::default()
        };
        match stats_asset_tokens::Entity::insert(model)
            .exec(self.db.as_ref())
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!(
                    err = ?e,
                    stats_asset_id,
                    chain_id,
                    "Failed to link token to stats asset"
                );
                Err(e.into())
            }
        }
    }

    pub async fn get_stats_asset_by_id(
        &self,
        id: i64,
    ) -> anyhow::Result<Option<stats_assets::Model>> {
        match stats_assets::Entity::find_by_id(id)
            .one(self.db.as_ref())
            .await
        {
            Ok(r) => Ok(r),
            Err(e) => {
                tracing::error!(err = ?e, id, "Failed to fetch stats asset by id");
                Err(e.into())
            }
        }
    }

    pub async fn get_stats_asset_by_token(
        &self,
        chain_id: i64,
        token_address: &[u8],
    ) -> anyhow::Result<Option<stats_assets::Model>> {
        let token_row = stats_asset_tokens::Entity::find()
            .filter(stats_asset_tokens::Column::ChainId.eq(chain_id))
            .filter(stats_asset_tokens::Column::TokenAddress.eq(token_address.to_vec()))
            .one(self.db.as_ref())
            .await?;
        let Some(t) = token_row else {
            return Ok(None);
        };
        stats_assets::Entity::find_by_id(t.stats_asset_id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| {
                tracing::error!(
                    err = ?e,
                    stats_asset_id = t.stats_asset_id,
                    "Failed to fetch stats asset by token"
                );
                e.into()
            })
    }

    /// Creates or updates a stats asset edge: on insert sets transfers_count=1 and cumulative_amount;
    /// on conflict increments transfers_count and adds to cumulative_amount. Preserves `amount_side`.
    pub async fn create_or_update_stats_asset_edge(
        &self,
        stats_asset_id: i64,
        src_chain_id: i64,
        dst_chain_id: i64,
        amount: sea_orm::prelude::BigDecimal,
        amount_side: EdgeAmountSide,
        decimals: Option<i16>,
    ) -> anyhow::Result<()> {
        let existing =
            stats_asset_edges::Entity::find_by_id((stats_asset_id, src_chain_id, dst_chain_id))
                .one(self.db.as_ref())
                .await?;

        if existing.is_some() {
            stats_asset_edges::Entity::update_many()
                .col_expr(
                    stats_asset_edges::Column::TransfersCount,
                    Expr::col(stats_asset_edges::Column::TransfersCount).add(1),
                )
                .col_expr(
                    stats_asset_edges::Column::CumulativeAmount,
                    Expr::col(stats_asset_edges::Column::CumulativeAmount).add(amount),
                )
                .col_expr(
                    stats_asset_edges::Column::UpdatedAt,
                    Expr::current_timestamp().into(),
                )
                .filter(stats_asset_edges::Column::StatsAssetId.eq(stats_asset_id))
                .filter(stats_asset_edges::Column::SrcChainId.eq(src_chain_id))
                .filter(stats_asset_edges::Column::DstChainId.eq(dst_chain_id))
                .exec(self.db.as_ref())
                .await
                .map_err(|e| {
                    tracing::error!(
                        err = ?e,
                        stats_asset_id,
                        src_chain_id,
                        dst_chain_id,
                        "Failed to update stats asset edge"
                    );
                    e
                })?;
        } else {
            let model = stats_asset_edges::ActiveModel {
                stats_asset_id: ActiveValue::Set(stats_asset_id),
                src_chain_id: ActiveValue::Set(src_chain_id),
                dst_chain_id: ActiveValue::Set(dst_chain_id),
                transfers_count: ActiveValue::Set(1),
                cumulative_amount: ActiveValue::Set(amount),
                decimals: ActiveValue::Set(decimals),
                amount_side: ActiveValue::Set(amount_side),
                ..Default::default()
            };
            stats_asset_edges::Entity::insert(model)
                .exec(self.db.as_ref())
                .await
                .map_err(|e| {
                    tracing::error!(
                        err = ?e,
                        stats_asset_id,
                        src_chain_id,
                        dst_chain_id,
                        "Failed to insert stats asset edge"
                    );
                    e
                })?;
        }
        Ok(())
    }

    /// Updates decimals for an existing edge. Does not change `amount_side`.
    pub async fn update_edge_decimals(
        &self,
        stats_asset_id: i64,
        src_chain_id: i64,
        dst_chain_id: i64,
        decimals: i16,
    ) -> anyhow::Result<()> {
        let res = stats_asset_edges::Entity::update_many()
            .col_expr(stats_asset_edges::Column::Decimals, Expr::value(decimals))
            .col_expr(
                stats_asset_edges::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(stats_asset_edges::Column::StatsAssetId.eq(stats_asset_id))
            .filter(stats_asset_edges::Column::SrcChainId.eq(src_chain_id))
            .filter(stats_asset_edges::Column::DstChainId.eq(dst_chain_id))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                tracing::error!(
                    err = ?e,
                    stats_asset_id,
                    src_chain_id,
                    dst_chain_id,
                    "Failed to update edge decimals"
                );
                e
            })?;
        if res.rows_affected == 0 {
            tracing::warn!(
                stats_asset_id,
                src_chain_id,
                dst_chain_id,
                "update_edge_decimals: no row updated"
            );
        }
        Ok(())
    }

    pub async fn upsert_stats_chains(
        &self,
        chain_id: i64,
        unique_transfer_users_count: i64,
        unique_message_users_count: i64,
    ) -> anyhow::Result<()> {
        let model = stats_chains::ActiveModel {
            chain_id: ActiveValue::Set(chain_id),
            unique_transfer_users_count: ActiveValue::Set(unique_transfer_users_count),
            unique_message_users_count: ActiveValue::Set(unique_message_users_count),
            ..Default::default()
        };
        match stats_chains::Entity::insert(model)
            .on_conflict(
                OnConflict::column(stats_chains::Column::ChainId)
                    .update_columns([
                        stats_chains::Column::UniqueTransferUsersCount,
                        stats_chains::Column::UniqueMessageUsersCount,
                    ])
                    .value(stats_chains::Column::UpdatedAt, Expr::current_timestamp())
                    .to_owned(),
            )
            .exec(self.db.as_ref())
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!(err = ?e, chain_id, "Failed to upsert stats_chains");
                Err(e.into())
            }
        }
    }

    /// Full refresh of `stats_chains` from `crosschain_messages` and `crosschain_transfers`.
    ///
    /// Counts distinct `(chain_id, address)` with **UNION** semantics between sender and recipient
    /// roles (no status filter, no transfer–message join). Runs in a single transaction.
    ///
    /// Implementation: `DELETE` all `stats_chains` rows in this transaction, then batch-insert
    /// the recomputed snapshot (`ON CONFLICT` matches insert-only after the delete).
    pub async fn recompute_stats_chains(&self) -> anyhow::Result<()> {
        #[derive(Debug, FromQueryResult)]
        struct ChainUserCountRow {
            chain_id: i64,
            user_count: i64,
        }

        let txn = self.db.begin().await?;
        let backend = txn.get_database_backend();

        let message_rows = ChainUserCountRow::find_by_statement(StatementBuilder::build(
            &select_stats_chains_message_user_counts(),
            &backend,
        ))
        .all(&txn)
        .await?;

        let transfer_rows = ChainUserCountRow::find_by_statement(StatementBuilder::build(
            &select_stats_chains_transfer_user_counts(),
            &backend,
        ))
        .all(&txn)
        .await?;

        let mut message_by_chain: HashMap<i64, i64> = HashMap::new();
        for r in message_rows {
            message_by_chain.insert(r.chain_id, r.user_count);
        }
        let mut transfer_by_chain: HashMap<i64, i64> = HashMap::new();
        for r in transfer_rows {
            transfer_by_chain.insert(r.chain_id, r.user_count);
        }

        let mut chain_ids_set: BTreeSet<i64> = BTreeSet::new();
        chain_ids_set.extend(message_by_chain.keys().copied());
        chain_ids_set.extend(transfer_by_chain.keys().copied());
        let chain_ids: Vec<i64> = chain_ids_set.into_iter().collect();

        stats_chains::Entity::delete_many().exec(&txn).await?;

        let models: Vec<stats_chains::ActiveModel> = chain_ids
            .iter()
            .map(|chain_id| stats_chains::ActiveModel {
                chain_id: ActiveValue::Set(*chain_id),
                unique_transfer_users_count: ActiveValue::Set(
                    *transfer_by_chain.get(chain_id).unwrap_or(&0),
                ),
                unique_message_users_count: ActiveValue::Set(
                    *message_by_chain.get(chain_id).unwrap_or(&0),
                ),
                ..Default::default()
            })
            .collect();

        if !models.is_empty() {
            let on_conflict = OnConflict::column(stats_chains::Column::ChainId)
                .update_columns([
                    stats_chains::Column::UniqueTransferUsersCount,
                    stats_chains::Column::UniqueMessageUsersCount,
                ])
                .value(stats_chains::Column::UpdatedAt, Expr::current_timestamp())
                .to_owned();
            crate::bulk::batched_upsert(&txn, &models, on_conflict).await?;
        }

        txn.commit().await?;
        Ok(())
    }

    /// Creates or increments the directional message count from src_chain_id to dst_chain_id.
    /// Insert with messages_count=1; on conflict increment messages_count and update updated_at.
    pub async fn create_or_update_stats_messages(
        &self,
        src_chain_id: i64,
        dst_chain_id: i64,
        messages_delta: i64,
    ) -> anyhow::Result<()> {
        let model = stats_messages::ActiveModel {
            src_chain_id: ActiveValue::Set(src_chain_id),
            dst_chain_id: ActiveValue::Set(dst_chain_id),
            messages_count: ActiveValue::Set(messages_delta),
            ..Default::default()
        };
        match stats_messages::Entity::insert(model)
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
            .exec(self.db.as_ref())
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!(
                    err = ?e,
                    src_chain_id,
                    dst_chain_id,
                    "Failed to create or update stats_messages"
                );
                Err(e.into())
            }
        }
    }

    /// Returns the stats_messages row for the given (src_chain_id, dst_chain_id), if any.
    pub async fn get_stats_messages_row(
        &self,
        src_chain_id: i64,
        dst_chain_id: i64,
    ) -> anyhow::Result<Option<stats_messages::Model>> {
        match stats_messages::Entity::find_by_id((src_chain_id, dst_chain_id))
            .one(self.db.as_ref())
            .await
        {
            Ok(r) => Ok(r),
            Err(e) => {
                tracing::error!(
                    err = ?e,
                    src_chain_id,
                    dst_chain_id,
                    "Failed to fetch stats_messages row"
                );
                Err(e.into())
            }
        }
    }

    pub async fn get_outgoing_message_paths(
        &self,
        chain_id: i64,
        from_date: Option<NaiveDate>,
        to_date: Option<NaiveDate>,
        counterparty_chain_ids: Option<&[i64]>,
        include_zero_chains: bool,
    ) -> anyhow::Result<Vec<MessagePathStatsRow>> {
        self.get_message_paths(
            chain_id,
            from_date,
            to_date,
            MessagePathDirection::Outgoing,
            counterparty_chain_ids,
            include_zero_chains,
        )
        .await
    }

    pub async fn get_incoming_message_paths(
        &self,
        chain_id: i64,
        from_date: Option<NaiveDate>,
        to_date: Option<NaiveDate>,
        counterparty_chain_ids: Option<&[i64]>,
        include_zero_chains: bool,
    ) -> anyhow::Result<Vec<MessagePathStatsRow>> {
        self.get_message_paths(
            chain_id,
            from_date,
            to_date,
            MessagePathDirection::Incoming,
            counterparty_chain_ids,
            include_zero_chains,
        )
        .await
    }

    async fn get_message_paths(
        &self,
        chain_id: i64,
        from_date: Option<NaiveDate>,
        to_date: Option<NaiveDate>,
        direction: MessagePathDirection,
        counterparty_chain_ids: Option<&[i64]>,
        include_zero_chains: bool,
    ) -> anyhow::Result<Vec<MessagePathStatsRow>> {
        if let (Some(from_date), Some(to_date)) = (from_date, to_date)
            && from_date >= to_date
        {
            return Ok(Vec::new());
        }

        let (sql, values) = match (from_date, to_date) {
            (None, None) => build_all_time_message_paths_query(
                chain_id,
                direction,
                counterparty_chain_ids,
                include_zero_chains,
            ),
            _ => build_bounded_message_paths_query(
                chain_id,
                from_date,
                to_date,
                direction,
                counterparty_chain_ids,
                include_zero_chains,
            ),
        };
        let stmt = Statement::from_sql_and_values(DatabaseBackend::Postgres, sql, values);

        let raw = self.db.query_all(stmt).await?;
        let mut rows = Vec::with_capacity(raw.len());
        for row in raw {
            rows.push(MessagePathStatsRow::from_query_result(&row, "")?);
        }

        Ok(rows)
    }

    /// Assigns a stats asset to a transfer. Transfer may keep stats_asset_id = NULL.
    pub async fn assign_transfer_stats_asset(
        &self,
        transfer_id: i64,
        stats_asset_id: Option<i64>,
    ) -> anyhow::Result<()> {
        let transfer = match crosschain_transfers::Entity::find_by_id(transfer_id)
            .one(self.db.as_ref())
            .await?
        {
            Some(t) => t,
            None => {
                tracing::error!(transfer_id, "Transfer not found for stats_asset_id assign");
                return Err(anyhow::anyhow!("Transfer {} not found", transfer_id));
            }
        };
        let mut am: crosschain_transfers::ActiveModel = transfer.into();
        am.stats_asset_id = ActiveValue::Set(stats_asset_id);
        am.update(self.db.as_ref()).await.map_err(|e| {
            tracing::error!(err = ?e, transfer_id, "Failed to assign transfer stats_asset_id");
            e
        })?;
        Ok(())
    }

    /// Increments `stats_processed` by 1 for the given message and sets `updated_at`. Fails if the row does not exist.
    pub async fn increment_message_stats_processed(
        &self,
        message_id: i64,
        bridge_id: i32,
    ) -> anyhow::Result<()> {
        let res = crosschain_messages::Entity::update_many()
            .col_expr(
                crosschain_messages::Column::StatsProcessed,
                Expr::col(crosschain_messages::Column::StatsProcessed).add(1),
            )
            .col_expr(
                crosschain_messages::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(crosschain_messages::Column::Id.eq(message_id))
            .filter(crosschain_messages::Column::BridgeId.eq(bridge_id))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                tracing::error!(
                    err = ?e,
                    message_id,
                    bridge_id,
                    "Failed to increment message stats_processed"
                );
                e
            })?;
        if res.rows_affected == 0 {
            tracing::error!(
                message_id,
                bridge_id,
                "Message not found for stats_processed increment"
            );
            return Err(anyhow::anyhow!(
                "Message ({}, {}) not found",
                message_id,
                bridge_id
            ));
        }
        Ok(())
    }

    /// Increments `stats_processed` by 1 for the given transfer and sets `updated_at`. Fails if the row does not exist.
    pub async fn increment_transfer_stats_processed(&self, transfer_id: i64) -> anyhow::Result<()> {
        let res = crosschain_transfers::Entity::update_many()
            .col_expr(
                crosschain_transfers::Column::StatsProcessed,
                Expr::col(crosschain_transfers::Column::StatsProcessed).add(1),
            )
            .col_expr(
                crosschain_transfers::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(crosschain_transfers::Column::Id.eq(transfer_id))
            .exec(self.db.as_ref())
            .await
            .map_err(|e| {
                tracing::error!(
                    err = ?e,
                    transfer_id,
                    "Failed to increment transfer stats_processed"
                );
                e
            })?;
        if res.rows_affected == 0 {
            tracing::error!(
                transfer_id,
                "Transfer not found for stats_processed increment"
            );
            return Err(anyhow::anyhow!("Transfer {} not found", transfer_id));
        }
        Ok(())
    }

    /// One backfill pass: process up to `message_limit` completed messages and
    /// `transfer_limit` transfers with `stats_processed = 0`. Uses the same projection
    /// as inline processing; each batch is one transaction.
    pub async fn backfill_stats_projection_round(
        &self,
        message_limit: u64,
        transfer_limit: u64,
    ) -> anyhow::Result<BackfillStatsReport> {
        let mut report = BackfillStatsReport::default();

        let msg_rows = crosschain_messages::Entity::find()
            .filter(crosschain_messages::Column::StatsProcessed.eq(0i16))
            .filter(crosschain_messages::Column::Status.eq(MessageStatus::Completed))
            .filter(crosschain_messages::Column::DstChainId.is_not_null())
            .order_by_asc(crosschain_messages::Column::Id)
            .limit(message_limit)
            .all(self.db.as_ref())
            .await
            .map_err(|e| {
                tracing::error!(err = ?e, "backfill_stats: list messages");
                e
            })?;

        if !msg_rows.is_empty() {
            let pks: Vec<(i64, i32)> = msg_rows.iter().map(|m| (m.id, m.bridge_id)).collect();
            let n = msg_rows.len();
            self.db
                .as_ref()
                .transaction(|tx| {
                    let pks = pks.clone();
                    Box::pin(async move {
                        crate::stats::projection::project_messages_batch(tx, &pks)
                            .await
                            .map(|_| ())
                    })
                })
                .await
                .map_err(|e| {
                    tracing::error!(err = ?e, "backfill_stats: message transaction");
                    anyhow::anyhow!("{}", e)
                })?;
            report.messages_processed = n;
        }

        let xfer_rows = crosschain_transfers::Entity::find()
            .join(
                JoinType::InnerJoin,
                crosschain_transfers::Relation::CrosschainMessages.def(),
            )
            .filter(crosschain_messages::Column::Status.eq(MessageStatus::Completed))
            .filter(crosschain_messages::Column::StatsProcessed.gt(0i16))
            .filter(crosschain_transfers::Column::StatsProcessed.eq(0i16))
            .order_by_asc(crosschain_transfers::Column::Id)
            .limit(transfer_limit)
            .all(self.db.as_ref())
            .await
            .map_err(|e| {
                tracing::error!(err = ?e, "backfill_stats: list transfers");
                e
            })?;

        if !xfer_rows.is_empty() {
            let ids: Vec<i64> = xfer_rows.iter().map(|t| t.id).collect();
            let n = xfer_rows.len();
            self.db
                .as_ref()
                .transaction(|tx| {
                    let ids = ids.clone();
                    Box::pin(async move {
                        crate::stats::projection::project_transfers_batch(tx, &ids)
                            .await
                            .map(|_| ())
                    })
                })
                .await
                .map_err(|e| {
                    tracing::error!(err = ?e, "backfill_stats: transfer transaction");
                    anyhow::anyhow!("{}", e)
                })?;
            report.transfers_processed = n;
            report.token_keys_for_enrichment =
                crate::stats::projection::token_keys_for_stats_enrichment_from_transfer_models(
                    &xfer_rows,
                );
        }

        Ok(report)
    }

    /// Runs [`Self::backfill_stats_projection_round`] until no eligible rows remain.
    /// Uses a fixed batch size per round (see [`STATS_BACKFILL_BATCH`]).
    pub async fn backfill_stats_until_idle(&self) -> anyhow::Result<()> {
        self.backfill_stats_until_idle_with_token_enrichment(None)
            .await
    }

    /// Like [`Self::backfill_stats_until_idle`], but after each successful transfer-projection
    /// batch (outside the DB transaction), kicks off non-blocking token fetches for missing
    /// metadata — same path as inline buffer flush.
    pub async fn backfill_stats_until_idle_with_token_enrichment(
        &self,
        token_info: Option<Arc<TokenInfoService>>,
    ) -> anyhow::Result<()> {
        let mut total_messages = 0usize;
        let mut total_transfers = 0usize;
        loop {
            let r = self
                .backfill_stats_projection_round(STATS_BACKFILL_BATCH, STATS_BACKFILL_BATCH)
                .await?;
            if r.messages_processed == 0 && r.transfers_processed == 0 {
                break;
            }
            total_messages += r.messages_processed;
            total_transfers += r.transfers_processed;
            tracing::info!(
                messages_this_round = r.messages_processed,
                transfers_this_round = r.transfers_processed,
                total_messages_so_far = total_messages,
                total_transfers_so_far = total_transfers,
                "stats backfill progress"
            );
            if let (Some(svc), keys) = (token_info.as_ref(), &r.token_keys_for_enrichment)
                && !keys.is_empty()
            {
                svc.clone()
                    .kickoff_token_fetch_for_stats_enrichment(keys.clone());
            }
        }
        tracing::info!(
            total_messages = total_messages,
            total_transfers = total_transfers,
            "stats backfill on start finished"
        );
        Ok(())
    }

    // CONFIGURATION TABLE: bridge_contracts
    pub async fn upsert_bridge_contracts(
        &self,
        bridge_contracts: Vec<bridge_contracts::ActiveModel>,
    ) -> anyhow::Result<()> {
        if bridge_contracts.is_empty() {
            return Ok(());
        }

        match bridge_contracts::Entity::insert_many(bridge_contracts)
            .on_conflict(
                OnConflict::columns([
                    bridge_contracts::Column::BridgeId,
                    bridge_contracts::Column::ChainId,
                    bridge_contracts::Column::Address,
                    bridge_contracts::Column::Version,
                ])
                .update_columns([
                    bridge_contracts::Column::Abi,
                    bridge_contracts::Column::StartedAtBlock,
                ])
                .value(
                    bridge_contracts::Column::UpdatedAt,
                    Expr::current_timestamp(),
                )
                .to_owned(),
            )
            .exec(self.db.as_ref())
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!(err =? e, "Failed to upsert bridge contracts");
                Err(e.into())
            }
        }
    }

    pub async fn get_bridge_contracts(
        &self,
        bridge_id: i32,
    ) -> anyhow::Result<Vec<bridge_contracts::Model>> {
        match bridge_contracts::Entity::find()
            .filter(bridge_contracts::Column::BridgeId.eq(bridge_id))
            .all(self.db.as_ref())
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => {
                tracing::error!(err =? e, "Failed to fetch bridge contracts");
                Err(e.into())
            }
        }
    }

    pub async fn get_bridge_contract(
        &self,
        bridge_id: i32,
        chain_id: i64,
    ) -> anyhow::Result<bridge_contracts::Model> {
        match bridge_contracts::Entity::find()
            .filter(bridge_contracts::Column::BridgeId.eq(bridge_id))
            .filter(bridge_contracts::Column::ChainId.eq(chain_id))
            .one(self.db.as_ref())
            .await
        {
            Ok(Some(result)) => Ok(result),
            Ok(None) => {
                let err_msg = format!(
                    "No bridge contract found for bridge_id={} and chain_id={}",
                    bridge_id, chain_id
                );
                tracing::error!("{}", err_msg);
                Err(anyhow::anyhow!(err_msg))
            }
            Err(e) => {
                tracing::error!(err =? e, "Failed to fetch bridge contract");
                Err(e.into())
            }
        }
    }

    // VIEW TABLE: crosschain_messages
    // Returns paginated list of crosschain messages with transfers for each message
    pub async fn get_crosschain_messages(
        &self,
        tx_hash: Option<Vec<u8>>,
        address: Option<Vec<u8>>,
        page_size: usize,
        last_page: bool,
        input_pagination: Option<MessagesPaginationLogic>,
    ) -> anyhow::Result<(
        Vec<(crosschain_messages::Model, Vec<crosschain_transfers::Model>)>,
        OutputPagination<MessagesPaginationLogic>,
    )> {
        let limit = page_size.max(1) as u64;

        let (items, pagination) = self
            .db
            .transaction(|tx| {
                //let input_pagination = input_pagination; // move into async block
                Box::pin(async move {
                    // Determine requested direction: default is Next
                    let query_direction = if last_page {
                        // Request rows from the end of the table to get the last page
                        // input pagination is ignored in this case
                        PaginationDirection::Prev
                    } else {
                        // Default direction is Next
                        input_pagination
                            .map(|m| m.direction)
                            .unwrap_or(PaginationDirection::Next)
                    };

                    // Base query
                    let mut query = crosschain_messages::Entity::find();

                    // Apply keyset pagination if marker is provided (and not requested the last page)
                    if !last_page && let Some(marker) = input_pagination {
                        let marker_ts = marker.timestamp;
                        let marker_id = marker.message_id as i64;
                        let marker_bridge_id = marker.bridge_id as i32;

                        let cond = match query_direction {
                            PaginationDirection::Next => {
                                // Older messages: (ts, id, bridge_id) < marker
                                Expr::col(crosschain_messages::Column::InitTimestamp)
                                    .lt(marker_ts)
                                    .or(Expr::col(crosschain_messages::Column::InitTimestamp)
                                        .eq(marker_ts)
                                        .and(
                                            Expr::col(crosschain_messages::Column::Id)
                                                .lt(marker_id),
                                        ))
                                    .or(Expr::col(crosschain_messages::Column::InitTimestamp)
                                        .eq(marker_ts)
                                        .and(
                                            Expr::col(crosschain_messages::Column::Id)
                                                .eq(marker_id),
                                        )
                                        .and(
                                            Expr::col(crosschain_messages::Column::BridgeId)
                                                .lt(marker_bridge_id),
                                        ))
                            }
                            PaginationDirection::Prev => {
                                // Newer messages: (ts, id, bridge_id) > marker
                                Expr::col(crosschain_messages::Column::InitTimestamp)
                                    .gt(marker_ts)
                                    .or(Expr::col(crosschain_messages::Column::InitTimestamp)
                                        .eq(marker_ts)
                                        .and(
                                            Expr::col(crosschain_messages::Column::Id)
                                                .gt(marker_id),
                                        ))
                                    .or(Expr::col(crosschain_messages::Column::InitTimestamp)
                                        .eq(marker_ts)
                                        .and(
                                            Expr::col(crosschain_messages::Column::Id)
                                                .eq(marker_id),
                                        )
                                        .and(
                                            Expr::col(crosschain_messages::Column::BridgeId)
                                                .gt(marker_bridge_id),
                                        ))
                            }
                        };

                        query = query.filter(cond);
                    }

                    // Apply tx_hash filter if provided
                    if let Some(tx_hash) = tx_hash.clone() {
                        let tx_filter = Expr::col(crosschain_messages::Column::SrcTxHash)
                            .eq(tx_hash.clone())
                            .or(Expr::col(crosschain_messages::Column::DstTxHash).eq(tx_hash));

                        query = query.filter(tx_filter);
                    }

                    // Apply address filter if provided
                    if let Some(address) = address.clone() {
                        let address_filter = Expr::col(crosschain_messages::Column::SenderAddress)
                            .eq(address.clone())
                            .or(Expr::col(crosschain_messages::Column::RecipientAddress)
                                .eq(address));

                        query = query.filter(address_filter);
                    }

                    // Apply ordering depending on requested direction
                    match query_direction {
                        PaginationDirection::Next => {
                            // Newest first
                            query = query
                                .order_by_desc(crosschain_messages::Column::InitTimestamp)
                                .order_by_desc(crosschain_messages::Column::Id)
                                .order_by_desc(crosschain_messages::Column::BridgeId);
                        }
                        PaginationDirection::Prev => {
                            // We fetch newer messages in ascending order and will reverse later
                            query = query
                                .order_by_asc(crosschain_messages::Column::InitTimestamp)
                                .order_by_asc(crosschain_messages::Column::Id)
                                .order_by_asc(crosschain_messages::Column::BridgeId);
                        }
                    }

                    // Fetch one extra row to detect "has more"
                    let mut messages: Vec<crosschain_messages::Model> =
                        query.limit(limit + 1).all(tx).await?;

                    let has_more = messages.len() as u64 > limit;

                    if has_more {
                        messages.truncate(limit as usize);
                    }

                    // For Prev we fetched in ascending order, but external API expects
                    // consistent "newest first" order, so reverse here.
                    if matches!(query_direction, PaginationDirection::Prev) {
                        messages.reverse();
                    }

                    // Load transfers for each message
                    let mut result: Vec<(
                        crosschain_messages::Model,
                        Vec<crosschain_transfers::Model>,
                    )> = Vec::with_capacity(messages.len());

                    for msg in &messages {
                        let transfers = crosschain_transfers::Entity::find()
                            .filter(crosschain_transfers::Column::MessageId.eq(msg.id))
                            .filter(crosschain_transfers::Column::BridgeId.eq(msg.bridge_id))
                            .all(tx)
                            .await?;

                        result.push((msg.clone(), transfers));
                    }

                    let mut pagination = build_pagination_from_messages(
                        &messages,
                        query_direction,
                        has_more,
                        last_page,
                    );

                    if tx_hash.is_some() && input_pagination.is_none() {
                        // Remove prev marker for a static list of messages
                        // (we assume there are no more new messages appearing after the initial request)
                        pagination = OutputPagination {
                            prev_marker: None,
                            next_marker: pagination.next_marker,
                        };
                    }

                    Ok::<_, DbErr>((result, pagination))
                })
            })
            .await?;

        Ok((items, pagination))
    }

    pub async fn get_crosschain_message(
        &self,
        message_id: Vec<u8>,
    ) -> anyhow::Result<Option<(crosschain_messages::Model, Vec<crosschain_transfers::Model>)>>
    {
        self.db
            .transaction(|tx| {
                Box::pin(async move {
                    // the filter depends on the length of the message_id
                    let f = if message_id.len() > 8 {
                        // long IDs are always stored into the native_id column
                        Expr::col(crosschain_messages::Column::NativeId).eq(message_id)
                    } else {
                        // IDs which fit in 8 bytes are stored in the PK
                        // left-pad with zeros to 8 bytes and interpret as big-endian integer
                        let mut buf = [0u8; 8];
                        buf[(8 - message_id.len())..].copy_from_slice(message_id.as_slice());
                        Expr::col(crosschain_messages::Column::Id).eq(i64::from_be_bytes(buf))
                    };

                    let query = crosschain_messages::Entity::find().filter(f);

                    match query.one(tx).await {
                        Ok(Some(msg)) => {
                            let transfers = crosschain_transfers::Entity::find()
                                .filter(crosschain_transfers::Column::MessageId.eq(msg.id))
                                .filter(crosschain_transfers::Column::BridgeId.eq(msg.bridge_id))
                                .all(tx)
                                .await?;

                            Ok(Some((msg, transfers)))
                        }
                        Ok(None) => Ok(None),
                        Err(e) => Err(e),
                    }
                })
            })
            .await
            .map_err(|e| e.into())
    }

    // VIEW TABLE: crosschain_transfers
    pub async fn get_crosschain_transfers(
        &self,
        tx_hash: Option<Vec<u8>>,
        address: Option<Vec<u8>>,
        page_size: usize,
        last_page: bool,
        input_pagination: Option<TransfersPaginationLogic>,
    ) -> anyhow::Result<(
        Vec<JoinedTransfer>,
        OutputPagination<TransfersPaginationLogic>,
    )> {
        let limit = page_size.max(1) as u64;

        let (items, pagination) = self
            .db
            .transaction(|tx| {
                let pagination_marker = input_pagination;
                let tx_hash_filter = tx_hash.clone();
                let address_filter = address.clone();

                Box::pin(async move {
                    let query_direction = if last_page {
                        PaginationDirection::Prev
                    } else {
                        pagination_marker
                            .map(|p| p.direction)
                            .unwrap_or(PaginationDirection::Next)
                    };

                    let mut query = crosschain_transfers::Entity::find()
                        .join(
                            JoinType::InnerJoin,
                            crosschain_transfers::Relation::CrosschainMessages.def(),
                        )
                        .select_only()
                        .column(crosschain_transfers::Column::Id)
                        .column(crosschain_transfers::Column::MessageId)
                        .column(crosschain_transfers::Column::BridgeId)
                        .column(crosschain_transfers::Column::Index)
                        .column(crosschain_transfers::Column::Type)
                        .column(crosschain_transfers::Column::TokenSrcChainId)
                        .column(crosschain_transfers::Column::TokenDstChainId)
                        .column(crosschain_transfers::Column::SrcAmount)
                        .column(crosschain_transfers::Column::DstAmount)
                        .column(crosschain_transfers::Column::TokenSrcAddress)
                        .column(crosschain_transfers::Column::TokenDstAddress)
                        .column(crosschain_transfers::Column::SenderAddress)
                        .column(crosschain_transfers::Column::RecipientAddress)
                        .column(crosschain_transfers::Column::TokenIds)
                        .column(crosschain_messages::Column::Status)
                        .column(crosschain_messages::Column::InitTimestamp)
                        .column(crosschain_messages::Column::LastUpdateTimestamp)
                        .column(crosschain_messages::Column::NativeId)
                        .column(crosschain_messages::Column::SrcTxHash)
                        .column(crosschain_messages::Column::DstTxHash);

                    if !last_page && let Some(marker) = pagination_marker {
                        let cond = match query_direction {
                            PaginationDirection::Next => {
                                Expr::col(crosschain_messages::Column::InitTimestamp)
                                    .lt(marker.timestamp)
                                    .or(Expr::col(crosschain_messages::Column::InitTimestamp)
                                        .eq(marker.timestamp)
                                        .and(
                                            Expr::col(crosschain_transfers::Column::MessageId)
                                                .lt(marker.message_id as i64),
                                        ))
                                    .or(Expr::col(crosschain_messages::Column::InitTimestamp)
                                        .eq(marker.timestamp)
                                        .and(
                                            Expr::col(crosschain_transfers::Column::MessageId)
                                                .eq(marker.message_id as i64),
                                        )
                                        .and(
                                            Expr::col((
                                                crosschain_transfers::Entity,
                                                crosschain_transfers::Column::BridgeId,
                                            ))
                                            .lt(marker.bridge_id as i32),
                                        ))
                                    .or(Expr::col(crosschain_messages::Column::InitTimestamp)
                                        .eq(marker.timestamp)
                                        .and(
                                            Expr::col(crosschain_transfers::Column::MessageId)
                                                .eq(marker.message_id as i64),
                                        )
                                        .and(
                                            Expr::col((
                                                crosschain_transfers::Entity,
                                                crosschain_transfers::Column::BridgeId,
                                            ))
                                            .eq(marker.bridge_id as i32),
                                        )
                                        .and(
                                            Expr::col(crosschain_transfers::Column::Index)
                                                .lt(marker.index as i64),
                                        ))
                            }
                            PaginationDirection::Prev => {
                                Expr::col(crosschain_messages::Column::InitTimestamp)
                                    .gt(marker.timestamp)
                                    .or(Expr::col(crosschain_messages::Column::InitTimestamp)
                                        .eq(marker.timestamp)
                                        .and(
                                            Expr::col(crosschain_transfers::Column::MessageId)
                                                .gt(marker.message_id as i64),
                                        ))
                                    .or(Expr::col(crosschain_messages::Column::InitTimestamp)
                                        .eq(marker.timestamp)
                                        .and(
                                            Expr::col(crosschain_transfers::Column::MessageId)
                                                .eq(marker.message_id as i64),
                                        )
                                        .and(
                                            Expr::col((
                                                crosschain_transfers::Entity,
                                                crosschain_transfers::Column::BridgeId,
                                            ))
                                            .gt(marker.bridge_id as i32),
                                        ))
                                    .or(Expr::col(crosschain_messages::Column::InitTimestamp)
                                        .eq(marker.timestamp)
                                        .and(
                                            Expr::col(crosschain_transfers::Column::MessageId)
                                                .eq(marker.message_id as i64),
                                        )
                                        .and(
                                            Expr::col((
                                                crosschain_transfers::Entity,
                                                crosschain_transfers::Column::BridgeId,
                                            ))
                                            .eq(marker.bridge_id as i32),
                                        )
                                        .and(
                                            Expr::col(crosschain_transfers::Column::Index)
                                                .gt(marker.index as i64),
                                        ))
                            }
                        };

                        query = query.filter(cond);
                    }

                    if let Some(hash) = tx_hash_filter.as_ref() {
                        let tx_filter = Expr::col(crosschain_messages::Column::SrcTxHash)
                            .eq(hash.clone())
                            .or(Expr::col(crosschain_messages::Column::DstTxHash).eq(hash.clone()));

                        query = query.filter(tx_filter);
                    }

                    if let Some(address) = address_filter.as_ref() {
                        let address_filter = Expr::col((
                            crosschain_transfers::Entity,
                            crosschain_transfers::Column::SenderAddress,
                        ))
                        .eq(address.clone())
                        .or(Expr::col((
                            crosschain_transfers::Entity,
                            crosschain_transfers::Column::RecipientAddress,
                        ))
                        .eq(address.clone()));

                        query = query.filter(address_filter);
                    }

                    match query_direction {
                        PaginationDirection::Next => {
                            query = query
                                .order_by_desc(crosschain_messages::Column::InitTimestamp)
                                .order_by_desc(crosschain_transfers::Column::MessageId)
                                .order_by_desc(Expr::col((
                                    crosschain_transfers::Entity,
                                    crosschain_transfers::Column::BridgeId,
                                )))
                                .order_by_desc(Expr::col((
                                    crosschain_transfers::Entity,
                                    crosschain_transfers::Column::Index,
                                )));
                        }
                        PaginationDirection::Prev => {
                            query = query
                                .order_by_asc(crosschain_messages::Column::InitTimestamp)
                                .order_by_asc(crosschain_transfers::Column::MessageId)
                                .order_by_asc(Expr::col((
                                    crosschain_transfers::Entity,
                                    crosschain_transfers::Column::BridgeId,
                                )))
                                .order_by_asc(Expr::col((
                                    crosschain_transfers::Entity,
                                    crosschain_transfers::Column::Index,
                                )));
                        }
                    }

                    let mut transfers: Vec<JoinedTransfer> = query
                        .limit(limit + 1)
                        .into_model::<JoinedTransfer>()
                        .all(tx)
                        .await?;

                    let has_more = transfers.len() as u64 > limit;
                    if has_more {
                        transfers.truncate(limit as usize);
                    }

                    if matches!(query_direction, PaginationDirection::Prev) {
                        transfers.reverse();
                    }

                    let mut pagination = build_pagination_from_transfers(
                        &transfers,
                        query_direction,
                        has_more,
                        last_page,
                    );

                    if tx_hash_filter.is_some() && pagination_marker.is_none() {
                        pagination.prev_marker = None;
                    }

                    Ok::<_, DbErr>((transfers, pagination))
                })
            })
            .await?;

        Ok((items, pagination))
    }

    // INDEXER TABLE: indexer_checkpoints
    /// Get checkpoint for a specific bridge and chain
    pub async fn get_checkpoint(
        &self,
        bridge_id: u64,
        chain_id: u64,
    ) -> anyhow::Result<Option<indexer_checkpoints::Model>> {
        indexer_checkpoints::Entity::find()
            .filter(indexer_checkpoints::Column::BridgeId.eq(bridge_id as i64))
            .filter(indexer_checkpoints::Column::ChainId.eq(chain_id as i64))
            .one(self.db.as_ref())
            .await
            .inspect_err(|e| tracing::error!(err =? e, "failed to query checkpoint from database"))
            .map_err(|e| e.into())
    }

    pub async fn get_token_info(
        &self,
        chain_id: u64,
        address: Vec<u8>,
    ) -> anyhow::Result<Option<tokens::Model>> {
        tokens::Entity::find()
            .filter(tokens::Column::ChainId.eq(chain_id as i64))
            .filter(tokens::Column::Address.eq(address))
            .one(self.db.as_ref())
            .await
            .inspect_err(|e| tracing::error!(err =? e, "failed to query token info from database"))
            .map_err(|e| e.into())
    }

    pub async fn upsert_token_info(&self, token_info: tokens::ActiveModel) -> anyhow::Result<()> {
        tokens::Entity::insert(token_info)
            .on_conflict(
                OnConflict::columns([tokens::Column::ChainId, tokens::Column::Address])
                    .update_columns([
                        tokens::Column::Name,
                        tokens::Column::Symbol,
                        tokens::Column::Decimals,
                        tokens::Column::TokenIcon,
                    ])
                    .value(tokens::Column::UpdatedAt, Expr::current_timestamp())
                    .to_owned(),
            )
            .exec(self.db.as_ref())
            .await?;

        Ok(())
    }

    /// Push token metadata/decimals into `stats_assets` / `stats_asset_edges` for rows linked via
    /// `stats_asset_tokens`. Only fills empty stats fields; edge `decimals` only when NULL and
    /// `amount_side` matches this token's chain (source vs destination for aggregated amounts);
    /// logs and skips on conflicting decimals.
    pub async fn propagate_token_info_to_stats_tables(
        &self,
        chain_id: i64,
        address: &[u8],
        token: &tokens::Model,
    ) -> anyhow::Result<()> {
        fn empty_opt(s: &Option<String>) -> bool {
            s.as_ref().is_none_or(|t| t.trim().is_empty())
        }
        fn nonempty_opt(s: &Option<String>) -> bool {
            s.as_ref().is_some_and(|t| !t.trim().is_empty())
        }

        let addr = address.to_vec();
        let links = stats_asset_tokens::Entity::find()
            .filter(stats_asset_tokens::Column::ChainId.eq(chain_id))
            .filter(stats_asset_tokens::Column::TokenAddress.eq(addr))
            .all(self.db.as_ref())
            .await?;

        let now = chrono::Utc::now().naive_utc();

        for link in links {
            let aid = link.stats_asset_id;
            let Some(asset) = stats_assets::Entity::find_by_id(aid)
                .one(self.db.as_ref())
                .await?
            else {
                continue;
            };

            let mut name = asset.name.clone();
            let mut symbol = asset.symbol.clone();
            let mut icon = asset.icon_url.clone();
            let mut meta_changed = false;

            if empty_opt(&name) && nonempty_opt(&token.name) {
                name = token.name.clone();
                meta_changed = true;
            }
            if empty_opt(&symbol) && nonempty_opt(&token.symbol) {
                symbol = token.symbol.clone();
                meta_changed = true;
            }
            if empty_opt(&icon) && nonempty_opt(&token.token_icon) {
                icon = token.token_icon.clone();
                meta_changed = true;
            }

            if meta_changed {
                stats_assets::Entity::update(stats_assets::ActiveModel {
                    id: ActiveValue::Unchanged(aid),
                    name: ActiveValue::Set(name),
                    symbol: ActiveValue::Set(symbol),
                    icon_url: ActiveValue::Set(icon),
                    created_at: ActiveValue::Unchanged(asset.created_at),
                    updated_at: ActiveValue::Set(now),
                })
                .exec(self.db.as_ref())
                .await?;
            }

            let edges = stats_asset_edges::Entity::find()
                .filter(stats_asset_edges::Column::StatsAssetId.eq(aid))
                .all(self.db.as_ref())
                .await?;

            for edge in edges {
                let amount_side_matches_chain = match edge.amount_side {
                    EdgeAmountSide::Source => edge.src_chain_id == chain_id,
                    EdgeAmountSide::Destination => edge.dst_chain_id == chain_id,
                };
                if !amount_side_matches_chain {
                    continue;
                }
                let Some(td) = token.decimals else {
                    continue;
                };
                match edge.decimals {
                    None => {
                        stats_asset_edges::Entity::update_many()
                            .col_expr(stats_asset_edges::Column::Decimals, Expr::value(td))
                            .col_expr(
                                stats_asset_edges::Column::UpdatedAt,
                                Expr::current_timestamp().into(),
                            )
                            .filter(stats_asset_edges::Column::StatsAssetId.eq(edge.stats_asset_id))
                            .filter(stats_asset_edges::Column::SrcChainId.eq(edge.src_chain_id))
                            .filter(stats_asset_edges::Column::DstChainId.eq(edge.dst_chain_id))
                            .exec(self.db.as_ref())
                            .await?;
                    }
                    Some(existing) if existing != td => {
                        tracing::warn!(
                            stats_asset_id = edge.stats_asset_id,
                            src_chain_id = edge.src_chain_id,
                            dst_chain_id = edge.dst_chain_id,
                            existing,
                            incoming = td,
                            "stats enrichment: not overwriting edge decimals (conflict)"
                        );
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Updates the token icon URL for a specific token.
    pub async fn update_token_icon(
        &self,
        chain_id: u64,
        address: Vec<u8>,
        icon_url: Option<String>,
    ) -> anyhow::Result<()> {
        tokens::Entity::update_many()
            .col_expr(tokens::Column::TokenIcon, Expr::value(icon_url))
            .col_expr(tokens::Column::UpdatedAt, Expr::current_timestamp().into())
            .filter(tokens::Column::ChainId.eq(chain_id as i64))
            .filter(tokens::Column::Address.eq(address))
            .exec(self.db.as_ref())
            .await
            .inspect_err(|e| tracing::error!(err =? e, "Failed to update token icon"))
            .map(|_| ())
            .map_err(|e| e.into())
    }

    /// Statistics
    pub async fn get_total_counters(
        &self,
        timestamp: NaiveDateTime,
        src_chain_filter: Option<u64>,
        dst_chain_filter: Option<u64>,
    ) -> anyhow::Result<InterchainTotalCounters> {
        self.db
            .transaction::<_, InterchainTotalCounters, DbErr>(|tx| {
                Box::pin(async move {
                    let mut filter = Condition::all()
                        .add(Expr::col(crosschain_messages::Column::InitTimestamp).lt(timestamp));

                    if let Some(src_chain_id) = src_chain_filter {
                        filter = filter.add(
                            Expr::col(crosschain_messages::Column::SrcChainId)
                                .eq(src_chain_id as i64),
                        );
                    }

                    if let Some(dst_chain_id) = dst_chain_filter {
                        filter = filter.add(
                            Expr::col(crosschain_messages::Column::DstChainId)
                                .eq(dst_chain_id as i64),
                        );
                    }

                    let total_messages = crosschain_messages::Entity::find()
                        .filter(filter.clone())
                        .count(tx)
                        .await?;

                    let total_transfers = crosschain_transfers::Entity::find()
                        .join(
                            JoinType::InnerJoin,
                            crosschain_transfers::Relation::CrosschainMessages.def(),
                        )
                        .filter(filter)
                        .count(tx)
                        .await?;

                    Ok(InterchainTotalCounters {
                        timestamp,
                        total_messages,
                        total_transfers,
                    })
                })
            })
            .await
            .map_err(|e| e.into())
    }

    pub async fn get_daily_counters(
        &self,
        timestamp: NaiveDateTime,
        src_chain_filter: Option<u64>,
        dst_chain_filter: Option<u64>,
    ) -> anyhow::Result<InterchainDailyCounters> {
        let day = timestamp.date();
        let day_start = day.and_hms_opt(0, 0, 0).expect("valid day start");
        let next_day_start = day_start + Duration::days(1);

        let mut filter = Condition::all()
            .add(Expr::col(crosschain_messages::Column::InitTimestamp).gte(day_start))
            .add(Expr::col(crosschain_messages::Column::InitTimestamp).lt(next_day_start));

        if let Some(src_chain_id) = src_chain_filter {
            filter = filter
                .add(Expr::col(crosschain_messages::Column::SrcChainId).eq(src_chain_id as i64));
        }

        if let Some(dst_chain_id) = dst_chain_filter {
            filter = filter
                .add(Expr::col(crosschain_messages::Column::DstChainId).eq(dst_chain_id as i64));
        }

        #[derive(Debug, FromQueryResult)]
        struct DailyCountersResult {
            daily_messages: i64,
            daily_transfers: i64,
        }

        // Single query: count distinct messages and total transfers via LEFT JOIN
        // COUNT(DISTINCT (m.id, m.bridge_id)) for messages (primary key)
        // COUNT(t.id) for transfers (NULL when no transfer exists)
        let result = crosschain_messages::Entity::find()
            .select_only()
            .column_as(
                Expr::expr(Func::count_distinct(Expr::tuple([
                    Expr::col((crosschain_messages::Entity, crosschain_messages::Column::Id))
                        .into(),
                    Expr::col((
                        crosschain_messages::Entity,
                        crosschain_messages::Column::BridgeId,
                    ))
                    .into(),
                ]))),
                "daily_messages",
            )
            .column_as(
                Expr::expr(Func::count(Expr::col((
                    crosschain_transfers::Entity,
                    crosschain_transfers::Column::Id,
                )))),
                "daily_transfers",
            )
            .join(
                JoinType::LeftJoin,
                crosschain_messages::Relation::CrosschainTransfers.def(),
            )
            .filter(filter)
            .into_model::<DailyCountersResult>()
            .one(self.db.as_ref())
            .await?
            .unwrap_or(DailyCountersResult {
                daily_messages: 0,
                daily_transfers: 0,
            });

        Ok(InterchainDailyCounters {
            date: day,
            daily_messages: result.daily_messages as u64,
            daily_transfers: result.daily_transfers as u64,
        })
    }

    // STAGING TABLE: pending_messages
    /// Insert or update a pending message (destination event arrived before source)
    pub async fn upsert_pending_message(
        &self,
        message: pending_messages::ActiveModel,
    ) -> anyhow::Result<()> {
        pending_messages::Entity::insert(message)
            .on_conflict(
                OnConflict::columns([
                    pending_messages::Column::MessageId,
                    pending_messages::Column::BridgeId,
                ])
                .update_columns([pending_messages::Column::Payload])
                .value(
                    pending_messages::Column::CreatedAt,
                    Expr::current_timestamp(),
                )
                .to_owned(),
            )
            .exec(self.db.as_ref())
            .await
            .inspect_err(|e| tracing::error!(err =? e, "Failed to upsert pending message"))
            .map(|_| ())
            .map_err(|e| e.into())
    }

    /// Get a crosschain message by primary key (message_id, bridge_id) with its transfers
    pub async fn get_crosschain_message_by_pk(
        &self,
        message_id: i64,
        bridge_id: i32,
    ) -> anyhow::Result<Option<(crosschain_messages::Model, Vec<crosschain_transfers::Model>)>>
    {
        let result = crosschain_messages::Entity::find_by_id((message_id, bridge_id))
            .find_with_related(crosschain_transfers::Entity)
            .all(self.db.as_ref())
            .await
            .inspect_err(|e| {
                tracing::error!(
                    err =? e,
                    message_id,
                    bridge_id,
                    "Failed to fetch crosschain message by PK"
                )
            })?;

        // find_with_related returns Vec<(Message, Vec<Transfer>)>, we only expect 0 or 1
        Ok(result.into_iter().next())
    }

    /// Check if a pending message exists for the given message_id and bridge_id
    pub async fn get_pending_message(
        &self,
        message_id: i64,
        bridge_id: i32,
    ) -> anyhow::Result<Option<pending_messages::Model>> {
        pending_messages::Entity::find()
            .filter(pending_messages::Column::MessageId.eq(message_id))
            .filter(pending_messages::Column::BridgeId.eq(bridge_id))
            .one(self.db.as_ref())
            .await
            .inspect_err(|e| {
                tracing::error!(
                    err =? e,
                    message_id,
                    bridge_id,
                    "Failed to fetch pending message"
                )
            })
            .map_err(|e| e.into())
    }

    /// Delete a pending message (called when both sides are found and message is promoted)
    pub async fn delete_pending_message(
        &self,
        message_id: i64,
        bridge_id: i32,
    ) -> anyhow::Result<()> {
        pending_messages::Entity::delete_many()
            .filter(pending_messages::Column::MessageId.eq(message_id))
            .filter(pending_messages::Column::BridgeId.eq(bridge_id))
            .exec(self.db.as_ref())
            .await
            .inspect_err(|e| {
                tracing::error!(
                    err =? e,
                    message_id,
                    bridge_id,
                    "Failed to delete pending message"
                )
            })
            .map(|_| ())
            .map_err(|e| e.into())
    }

    /// Paginated bridged-token statistics for `chain_id` (aggregated per `stats_asset`).
    pub async fn list_bridged_token_stats_for_chain(
        &self,
        chain_id: i64,
        params: crate::stats::StatsListQuery<
            '_,
            crate::pagination::BridgedTokensSortField,
            crate::pagination::BridgedTokensPaginationLogic,
        >,
    ) -> anyhow::Result<(
        Vec<crate::bridged_tokens_query::BridgedTokenAggDbRow>,
        OutputPagination<crate::pagination::BridgedTokensPaginationLogic>,
    )> {
        crate::bridged_tokens_query::list_bridged_token_stats_for_chain(
            self.db.as_ref(),
            chain_id,
            params,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    pub async fn list_stats_chains(
        &self,
        chain_ids: &[i64],
        include_zero_chains: bool,
        params: crate::stats::StatsListQuery<
            '_,
            crate::pagination::StatsChainsSortField,
            crate::pagination::StatsChainsPaginationLogic,
        >,
    ) -> anyhow::Result<(
        Vec<crate::stats_chains_query::StatsChainListRow>,
        OutputPagination<crate::pagination::StatsChainsPaginationLogic>,
    )> {
        crate::stats_chains_query::list_stats_chains(
            self.db.as_ref(),
            chain_ids,
            include_zero_chains,
            params,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    pub async fn fetch_bridged_token_items_for_assets(
        &self,
        asset_ids: &[i64],
    ) -> anyhow::Result<
        std::collections::HashMap<i64, Vec<crate::bridged_tokens_query::BridgedTokenLinkEnriched>>,
    > {
        crate::bridged_tokens_query::fetch_bridged_token_items_for_assets(
            self.db.as_ref(),
            asset_ids,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))
    }
}

/// Build OutputPagination from a page of messages.
/// prev_marker and next_marker are built from the first and last element (if exists) respectively.
/// We must take into account a few query parameters.
fn build_pagination_from_messages(
    messages: &[crosschain_messages::Model],
    query_direction: PaginationDirection,
    has_more: bool,
    last_page: bool,
) -> OutputPagination<MessagesPaginationLogic> {
    //We assume that new messages can appear in the database at any time,
    // so the prev marker should always be returned based on the first message
    // (except when there are no messages on the current page).
    let prev_marker = messages.first().map(|msg| MessagesPaginationLogic {
        timestamp: msg.init_timestamp,
        message_id: msg.id as u64,
        bridge_id: msg.bridge_id as u32,
        direction: PaginationDirection::Prev,
    });

    // The next marker should not be returned if the last page is requested
    // or if there are no more messages to fetch in the next direction.
    // When the query direction is prev (backward), we assume that
    // the next marker should always be returned.
    let next_marker = if !last_page && (query_direction == PaginationDirection::Prev || has_more) {
        messages.last().map(|msg| MessagesPaginationLogic {
            timestamp: msg.init_timestamp,
            message_id: msg.id as u64,
            bridge_id: msg.bridge_id as u32,
            direction: PaginationDirection::Next,
        })
    } else {
        None
    };

    OutputPagination {
        prev_marker,
        next_marker,
    }
}

/// Build OutputPagination from a page of transfers.
/// prev_marker and next_marker are built from the first and last element (if exists) respectively.
/// We must take into account a few query parameters.
fn build_pagination_from_transfers(
    transfers: &[JoinedTransfer],
    query_direction: PaginationDirection,
    has_more: bool,
    last_page: bool,
) -> OutputPagination<TransfersPaginationLogic> {
    //We assume that new messages can appear in the database at any time,
    // so the prev marker should always be returned based on the first message
    // (except when there are no messages on the current page).
    let prev_marker = transfers.first().map(|transfer| TransfersPaginationLogic {
        timestamp: transfer.init_timestamp,
        message_id: transfer.message_id as u64,
        bridge_id: transfer.bridge_id as u32,
        index: transfer.index as u64,
        direction: PaginationDirection::Prev,
    });

    // The next marker should not be returned if the last page is requested
    // or if there are no more messages to fetch in the next direction.
    // When the query direction is prev (backward), we assume that
    // the next marker should always be returned.
    let next_marker = if !last_page && (query_direction == PaginationDirection::Prev || has_more) {
        transfers.last().map(|transfer| TransfersPaginationLogic {
            timestamp: transfer.init_timestamp,
            message_id: transfer.message_id as u64,
            bridge_id: transfer.bridge_id as u32,
            index: transfer.index as u64,
            direction: PaginationDirection::Next,
        })
    } else {
        None
    };

    OutputPagination {
        prev_marker,
        next_marker,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Utc};
    use interchain_indexer_entity::{
        chains, crosschain_messages, crosschain_transfers,
        sea_orm_active_enums::{EdgeAmountSide, MessageStatus, TransferType},
        stats_asset_edges, stats_asset_tokens, stats_assets, stats_chains, stats_messages,
        stats_messages_days, tokens,
    };
    use sea_orm::{
        ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
        TransactionTrait, prelude::BigDecimal,
    };

    use crate::{
        InterchainDatabase, MessagePathStatsRow,
        test_utils::{init_db, mock_db::fill_mock_interchain_database},
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn mock_db_works() {
        let db = init_db("mock_db_works").await;
        fill_mock_interchain_database(&db).await;

        let interchain_db = InterchainDatabase::new(db.client());

        let chains = interchain_db.get_all_chains().await.unwrap();
        assert_eq!(chains.len(), 2);

        let bridges = interchain_db.get_all_bridges().await.unwrap();
        assert_eq!(bridges.len(), 1);

        let bridge_contracts = interchain_db
            .get_bridge_contracts(bridges[0].id)
            .await
            .unwrap();
        assert_eq!(bridge_contracts.len(), 2);

        let bridge_contract = interchain_db
            .get_bridge_contract(bridges[0].id, chains[0].id)
            .await
            .unwrap();
        assert_eq!(bridge_contract.id, bridge_contracts[0].id);
        assert_eq!(bridge_contract.chain_id, chains[0].id);
        assert_eq!(bridge_contract.bridge_id, bridges[0].id);
        assert_eq!(bridge_contract.address, bridge_contracts[0].address);

        let (crosschain_messages, _) = interchain_db
            .get_crosschain_messages(None, None, 100, false, None)
            .await
            .unwrap();
        assert_eq!(crosschain_messages.len(), 4);

        let crosschain_transfers = interchain_db
            .get_crosschain_transfers(None, None, 50, false, None)
            .await
            .unwrap();
        assert_eq!(crosschain_transfers.0.len(), 5);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn mock_db_upsert_chain() {
        let db = init_db("mock_db_upsert_chain").await;
        fill_mock_interchain_database(&db).await;

        let interchain_db = InterchainDatabase::new(db.client());

        let mut ava_chain = chains::ActiveModel {
            id: Set(43114),
            name: Set("C-Chain".to_string()),
            icon: Set(Some(
                "https://chainlist.org/chain/43114/icon.png".to_string(),
            )),
            ..Default::default()
        };

        interchain_db.upsert_chains(vec![]).await.unwrap();
        interchain_db
            .upsert_chains(vec![ava_chain.clone()])
            .await
            .unwrap();

        let chains = interchain_db.get_all_chains().await.unwrap();
        assert_eq!(chains.len(), 3);

        ava_chain.name = Set("Avalanche C-Chain".to_string());
        interchain_db
            .upsert_chains(vec![ava_chain.clone()])
            .await
            .unwrap();

        let chains = interchain_db.get_all_chains().await.unwrap();
        assert_eq!(chains.len(), 3);
        let stored_chain = chains.iter().find(|chain| chain.id == 43114).unwrap();
        assert_eq!(stored_chain.name, ava_chain.name.unwrap());
        assert_eq!(stored_chain.icon, ava_chain.icon.unwrap());
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn load_native_id_map_filters_missing_native_ids() {
        let db = init_db("load_native_id_map_filters_missing_native_ids").await;
        let interchain_db = InterchainDatabase::new(db.client());

        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("ChainA".to_string()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("ChainB".to_string()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("ChainC".to_string()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        // Create mappings for only some chains.
        interchain_indexer_entity::avalanche_icm_blockchain_ids::Entity::insert_many([
            interchain_indexer_entity::avalanche_icm_blockchain_ids::ActiveModel {
                blockchain_id: Set(vec![0xaa]),
                chain_id: Set(1),
                ..Default::default()
            },
            interchain_indexer_entity::avalanche_icm_blockchain_ids::ActiveModel {
                blockchain_id: Set(vec![0xbb]),
                chain_id: Set(2),
                ..Default::default()
            },
        ])
        .exec(interchain_db.db.as_ref())
        .await
        .unwrap();

        let map = interchain_db.load_native_id_map().await.unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("0xaa"), Some(&1));
        assert_eq!(map.get("0xbb"), Some(&2));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_crosschain_message_handles_pk_and_native_ids() {
        let db = init_db("get_crosschain_message_handles_pk_and_native_ids").await;
        fill_mock_interchain_database(&db).await;

        let interchain_db = InterchainDatabase::new(db.client());

        let (msg, transfers) = interchain_db
            .get_crosschain_message(1001i64.to_be_bytes().to_vec())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(msg.id, 1001);
        assert_eq!(transfers.len(), 1);

        let native_id = vec![9u8; 16];
        crosschain_messages::Entity::insert(crosschain_messages::ActiveModel {
            id: Set(2001),
            bridge_id: Set(1),
            status: Set(MessageStatus::Initiated),
            src_chain_id: Set(1),
            dst_chain_id: Set(Some(100)),
            native_id: Set(Some(native_id.clone())),
            ..Default::default()
        })
        .exec(interchain_db.db.as_ref())
        .await
        .unwrap();

        let (msg, transfers) = interchain_db
            .get_crosschain_message(native_id.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(msg.native_id, Some(native_id));
        assert!(transfers.is_empty());
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn counters_cover_all_filters() {
        let db = init_db("counters_cover_all_filters").await;
        fill_mock_interchain_database(&db).await;

        let interchain_db = InterchainDatabase::new(db.client());
        let ts = (Utc::now() + chrono::Duration::seconds(1)).naive_utc();

        let totals = interchain_db
            .get_total_counters(ts, None, None)
            .await
            .unwrap();
        assert_eq!(totals.total_messages, 4);
        assert_eq!(totals.total_transfers, 5);

        let src_filtered = interchain_db
            .get_total_counters(ts, Some(1), None)
            .await
            .unwrap();
        assert_eq!(src_filtered.total_messages, 2);
        assert_eq!(src_filtered.total_transfers, 3);

        let daily = interchain_db
            .get_daily_counters(ts, None, None)
            .await
            .unwrap();
        assert_eq!(daily.daily_messages, 4);
        assert_eq!(daily.daily_transfers, 5);

        let dst_filtered = interchain_db
            .get_daily_counters(ts, None, Some(100))
            .await
            .unwrap();
        assert_eq!(dst_filtered.daily_messages, 2);
        assert_eq!(dst_filtered.daily_transfers, 3);
    }

    // --- Stats assets migration and persistence tests ---

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_migration_applies() {
        let _db = init_db("stats_migration_applies").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        let asset = interchain_db
            .create_stats_asset(Some("Test".to_string()), Some("T".to_string()), None)
            .await
            .unwrap();
        assert!(asset.id > 0);
        assert_eq!(asset.name.as_deref(), Some("Test"));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_asset_insert_and_get() {
        let _db = init_db("stats_asset_insert_and_get").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![chains::ActiveModel {
                id: Set(1),
                name: Set("Chain1".to_string()),
                ..Default::default()
            }])
            .await
            .unwrap();

        let asset = interchain_db
            .create_stats_asset(Some("A".to_string()), Some("A".to_string()), None)
            .await
            .unwrap();
        let got = interchain_db
            .get_stats_asset_by_id(asset.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.id, asset.id);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_link_token_without_tokens_row() {
        let _db = init_db("stats_link_token_without_tokens_row").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![chains::ActiveModel {
                id: Set(1),
                name: Set("C1".to_string()),
                ..Default::default()
            }])
            .await
            .unwrap();
        let asset = interchain_db
            .create_stats_asset(Some("X".to_string()), Some("X".to_string()), None)
            .await
            .unwrap();
        let addr = vec![0xaa; 20];
        interchain_db
            .link_token_to_stats_asset(asset.id, 1, addr.clone())
            .await
            .unwrap();
        let found = interchain_db
            .get_stats_asset_by_token(1, &addr)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id, asset.id);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_reject_same_token_two_assets() {
        let _db = init_db("stats_reject_same_token_two_assets").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![chains::ActiveModel {
                id: Set(1),
                name: Set("C1".to_string()),
                ..Default::default()
            }])
            .await
            .unwrap();
        let a1 = interchain_db
            .create_stats_asset(Some("A1".to_string()), None, None)
            .await
            .unwrap();
        let a2 = interchain_db
            .create_stats_asset(Some("A2".to_string()), None, None)
            .await
            .unwrap();
        let addr = vec![0xbb; 20];
        interchain_db
            .link_token_to_stats_asset(a1.id, 1, addr.clone())
            .await
            .unwrap();
        let res = interchain_db
            .link_token_to_stats_asset(a2.id, 1, addr)
            .await;
        assert!(res.is_err());
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_reject_two_tokens_same_chain_one_asset() {
        let _db = init_db("stats_reject_two_tokens_same_chain_one_asset").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![chains::ActiveModel {
                id: Set(1),
                name: Set("C1".to_string()),
                ..Default::default()
            }])
            .await
            .unwrap();
        let asset = interchain_db
            .create_stats_asset(Some("A".to_string()), None, None)
            .await
            .unwrap();
        interchain_db
            .link_token_to_stats_asset(asset.id, 1, vec![1u8; 20])
            .await
            .unwrap();
        let res = interchain_db
            .link_token_to_stats_asset(asset.id, 1, vec![2u8; 20])
            .await;
        assert!(res.is_err());
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_edge_insert_and_upsert() {
        let _db = init_db("stats_edge_insert_and_upsert").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("C1".to_string()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("C2".to_string()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();
        let asset = interchain_db
            .create_stats_asset(Some("E".to_string()), None, None)
            .await
            .unwrap();
        let amount = BigDecimal::from(1000u64);
        interchain_db
            .create_or_update_stats_asset_edge(
                asset.id,
                1,
                2,
                amount.clone(),
                EdgeAmountSide::Source,
                Some(18),
            )
            .await
            .unwrap();
        let edge = stats_asset_edges::Entity::find_by_id((asset.id, 1i64, 2i64))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.transfers_count, 1);
        assert_eq!(edge.cumulative_amount, amount);
        assert_eq!(edge.amount_side, EdgeAmountSide::Source);
        assert_eq!(edge.decimals, Some(18));

        interchain_db
            .create_or_update_stats_asset_edge(
                asset.id,
                1,
                2,
                BigDecimal::from(500u64),
                EdgeAmountSide::Source,
                None,
            )
            .await
            .unwrap();
        let edge2 = stats_asset_edges::Entity::find_by_id((asset.id, 1i64, 2i64))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge2.transfers_count, 2);
        assert_eq!(edge2.cumulative_amount, BigDecimal::from(1500u64));
        assert_eq!(edge2.amount_side, EdgeAmountSide::Source);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_edge_decimals_null_and_update() {
        let _db = init_db("stats_edge_decimals_null_and_update").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("C1".to_string()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("C2".to_string()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();
        let asset = interchain_db
            .create_stats_asset(Some("D".to_string()), None, None)
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_asset_edge(
                asset.id,
                1,
                2,
                BigDecimal::from(1u64),
                EdgeAmountSide::Destination,
                None,
            )
            .await
            .unwrap();
        let edge = stats_asset_edges::Entity::find_by_id((asset.id, 1i64, 2i64))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.decimals, None);
        assert_eq!(edge.amount_side, EdgeAmountSide::Destination);

        interchain_db
            .update_edge_decimals(asset.id, 1, 2, 6)
            .await
            .unwrap();
        let edge2 = stats_asset_edges::Entity::find_by_id((asset.id, 1i64, 2i64))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge2.decimals, Some(6));
        assert_eq!(edge2.amount_side, EdgeAmountSide::Destination);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_chains_upsert() {
        let _db = init_db("stats_chains_upsert").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![chains::ActiveModel {
                id: Set(1),
                name: Set("C1".to_string()),
                ..Default::default()
            }])
            .await
            .unwrap();
        interchain_db.upsert_stats_chains(1, 10, 20).await.unwrap();
        let row = stats_chains::Entity::find_by_id(1i64)
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.unique_transfer_users_count, 10);
        assert_eq!(row.unique_message_users_count, 20);

        interchain_db.upsert_stats_chains(1, 30, 40).await.unwrap();
        let row2 = stats_chains::Entity::find_by_id(1i64)
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row2.unique_transfer_users_count, 30);
        assert_eq!(row2.unique_message_users_count, 40);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn recompute_stats_chains_distinct_users_and_merges_message_transfer_sides() {
        use alloy::primitives::address;
        use interchain_indexer_entity::{bridge_contracts, bridges};

        let _db = init_db("recompute_stats_chains_distinct").await;
        let conn = _db.client();
        let interchain_db = InterchainDatabase::new(conn.clone());

        let c1 = 90_001i64;
        let c2 = 90_002i64;
        let c3 = 90_003i64;
        let c4 = 90_004i64;
        let c5 = 90_005i64;
        let c6 = 90_006i64;
        let c7 = 90_007i64;
        let c_idle = 90_008i64;

        let addr_a = address!("0x0000000000000000000000000000000000000a01")
            .as_slice()
            .to_vec();
        let addr_b = address!("0x0000000000000000000000000000000000000b02")
            .as_slice()
            .to_vec();
        let addr_c = address!("0x0000000000000000000000000000000000000c03")
            .as_slice()
            .to_vec();
        let addr_same = address!("0x000000000000000000000000000000000000d00d")
            .as_slice()
            .to_vec();
        let addr_x = address!("0x0000000000000000000000000000000000000e04")
            .as_slice()
            .to_vec();
        let addr_y = address!("0x0000000000000000000000000000000000000f05")
            .as_slice()
            .to_vec();
        let addr_t1 = address!("0x0000000000000000000000000000000000000111")
            .as_slice()
            .to_vec();
        let addr_t2 = address!("0x0000000000000000000000000000000000000222")
            .as_slice()
            .to_vec();
        let token = address!("0x1111111111111111111111111111111111111111")
            .as_slice()
            .to_vec();

        chains::Entity::insert_many([
            chains::ActiveModel {
                id: Set(c1),
                name: Set("re_sc_c90001".to_string()),
                ..Default::default()
            },
            chains::ActiveModel {
                id: Set(c2),
                name: Set("re_sc_c90002".to_string()),
                ..Default::default()
            },
            chains::ActiveModel {
                id: Set(c3),
                name: Set("re_sc_c90003".to_string()),
                ..Default::default()
            },
            chains::ActiveModel {
                id: Set(c4),
                name: Set("re_sc_c90004".to_string()),
                ..Default::default()
            },
            chains::ActiveModel {
                id: Set(c5),
                name: Set("re_sc_c90005".to_string()),
                ..Default::default()
            },
            chains::ActiveModel {
                id: Set(c6),
                name: Set("re_sc_c90006".to_string()),
                ..Default::default()
            },
            chains::ActiveModel {
                id: Set(c7),
                name: Set("re_sc_c90007".to_string()),
                ..Default::default()
            },
            chains::ActiveModel {
                id: Set(c_idle),
                name: Set("re_sc_idle_no_stats".to_string()),
                ..Default::default()
            },
        ])
        .exec(conn.as_ref())
        .await
        .unwrap();

        bridges::Entity::insert(bridges::ActiveModel {
            id: Set(1),
            name: Set("recompute_stats_chains_bridge".to_string()),
            enabled: Set(true),
            ..Default::default()
        })
        .exec(conn.as_ref())
        .await
        .unwrap();

        bridge_contracts::Entity::insert_many([
            bridge_contracts::ActiveModel {
                bridge_id: Set(1),
                chain_id: Set(c1),
                address: Set(vec![0x11; 20]),
                ..Default::default()
            },
            bridge_contracts::ActiveModel {
                bridge_id: Set(1),
                chain_id: Set(c2),
                address: Set(vec![0x22; 20]),
                ..Default::default()
            },
        ])
        .exec(conn.as_ref())
        .await
        .unwrap();

        // Duplicate (c1, addr_a) as sender — counts once. (c2, addr_b) twice as recipient — once.
        crosschain_messages::Entity::insert_many([
            crosschain_messages::ActiveModel {
                id: Set(50_001),
                bridge_id: Set(1),
                status: Set(MessageStatus::Initiated),
                src_chain_id: Set(c1),
                dst_chain_id: Set(Some(c2)),
                sender_address: Set(Some(addr_a.clone())),
                recipient_address: Set(Some(addr_b.clone())),
                stats_processed: Set(0),
                ..Default::default()
            },
            crosschain_messages::ActiveModel {
                id: Set(50_002),
                bridge_id: Set(1),
                status: Set(MessageStatus::Completed),
                src_chain_id: Set(c1),
                dst_chain_id: Set(Some(c2)),
                sender_address: Set(Some(addr_a.clone())),
                recipient_address: Set(Some(addr_b.clone())),
                stats_processed: Set(0),
                ..Default::default()
            },
            // c2 as src (addr_a), c3 as dst (addr_c) — message users on c2 include prior dst addr_b plus addr_a.
            crosschain_messages::ActiveModel {
                id: Set(50_003),
                bridge_id: Set(1),
                status: Set(MessageStatus::Initiated),
                src_chain_id: Set(c2),
                dst_chain_id: Set(Some(c3)),
                sender_address: Set(Some(addr_a.clone())),
                recipient_address: Set(Some(addr_c.clone())),
                stats_processed: Set(0),
                ..Default::default()
            },
            // Same raw address on c4 (src) and c4 (dst from another hop) — one user on c4.
            crosschain_messages::ActiveModel {
                id: Set(50_004),
                bridge_id: Set(1),
                status: Set(MessageStatus::Initiated),
                src_chain_id: Set(c4),
                dst_chain_id: Set(Some(c5)),
                sender_address: Set(Some(addr_same.clone())),
                recipient_address: Set(Some(addr_x.clone())),
                stats_processed: Set(0),
                ..Default::default()
            },
            crosschain_messages::ActiveModel {
                id: Set(50_005),
                bridge_id: Set(1),
                status: Set(MessageStatus::Initiated),
                src_chain_id: Set(c5),
                dst_chain_id: Set(Some(c4)),
                sender_address: Set(Some(addr_y.clone())),
                recipient_address: Set(Some(addr_same.clone())),
                stats_processed: Set(0),
                ..Default::default()
            },
            // Failed status still counts toward stats_chains.
            crosschain_messages::ActiveModel {
                id: Set(50_006),
                bridge_id: Set(1),
                status: Set(MessageStatus::Failed),
                src_chain_id: Set(c1),
                dst_chain_id: Set(Some(c2)),
                sender_address: Set(Some(addr_a.clone())),
                recipient_address: Set(Some(addr_b.clone())),
                stats_processed: Set(0),
                ..Default::default()
            },
            // Carrier message for transfer-only user paths (null message addresses).
            crosschain_messages::ActiveModel {
                id: Set(50_007),
                bridge_id: Set(1),
                status: Set(MessageStatus::Initiated),
                src_chain_id: Set(c6),
                dst_chain_id: Set(Some(c7)),
                sender_address: Set(None),
                recipient_address: Set(None),
                stats_processed: Set(0),
                ..Default::default()
            },
        ])
        .exec(conn.as_ref())
        .await
        .unwrap();

        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: Set(90_001),
            message_id: Set(50_007),
            bridge_id: Set(1),
            index: Set(0),
            r#type: Set(Some(TransferType::Erc20)),
            token_src_chain_id: Set(c6),
            token_dst_chain_id: Set(c7),
            src_amount: Set(BigDecimal::from(1u64)),
            dst_amount: Set(BigDecimal::from(1u64)),
            token_src_address: Set(token.clone()),
            token_dst_address: Set(token.clone()),
            sender_address: Set(Some(addr_t1.clone())),
            recipient_address: Set(Some(addr_t2.clone())),
            stats_processed: Set(0),
            ..Default::default()
        })
        .exec(conn.as_ref())
        .await
        .unwrap();

        interchain_db
            .upsert_stats_chains(c1, 999, 888)
            .await
            .unwrap();

        interchain_db.recompute_stats_chains().await.unwrap();

        let r1 = stats_chains::Entity::find_by_id(c1)
            .one(conn.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r1.unique_message_users_count, 1, "c1: only addr_a");
        assert_eq!(r1.unique_transfer_users_count, 0);

        let r2 = stats_chains::Entity::find_by_id(c2)
            .one(conn.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            r2.unique_message_users_count, 2,
            "c2: addr_b as dst, addr_a as src"
        );
        assert_eq!(r2.unique_transfer_users_count, 0);

        let r3 = stats_chains::Entity::find_by_id(c3)
            .one(conn.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r3.unique_message_users_count, 1);
        assert_eq!(r3.unique_transfer_users_count, 0);

        let r4 = stats_chains::Entity::find_by_id(c4)
            .one(conn.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            r4.unique_message_users_count, 1,
            "c4: union of same address on src and dst"
        );
        assert_eq!(r4.unique_transfer_users_count, 0);

        let r5 = stats_chains::Entity::find_by_id(c5)
            .one(conn.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r5.unique_message_users_count, 2);
        assert_eq!(r5.unique_transfer_users_count, 0);

        let r6 = stats_chains::Entity::find_by_id(c6)
            .one(conn.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r6.unique_message_users_count, 0);
        assert_eq!(r6.unique_transfer_users_count, 1);

        let r7 = stats_chains::Entity::find_by_id(c7)
            .one(conn.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r7.unique_message_users_count, 0);
        assert_eq!(r7.unique_transfer_users_count, 1);

        assert_ne!(
            r1.unique_message_users_count, 888,
            "stale upsert should be replaced"
        );

        assert!(
            stats_chains::Entity::find_by_id(c_idle)
                .one(conn.as_ref())
                .await
                .unwrap()
                .is_none(),
            "configured chain with no message/transfer users must not get a stats_chains row"
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn transfer_stats_asset_id_null_and_set() {
        let _db = init_db("transfer_stats_asset_id_null_and_set").await;
        fill_mock_interchain_database(&_db).await;
        let interchain_db = InterchainDatabase::new(_db.client());
        let (transfers_list, _) = interchain_db
            .get_crosschain_transfers(None, None, 10, false, None)
            .await
            .unwrap();
        let transfer_id = transfers_list[0].id;
        let transfer_row = crosschain_transfers::Entity::find_by_id(transfer_id)
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(transfer_row.stats_asset_id, None);

        let asset = interchain_db
            .create_stats_asset(Some("T".to_string()), None, None)
            .await
            .unwrap();
        interchain_db
            .assign_transfer_stats_asset(transfer_id, Some(asset.id))
            .await
            .unwrap();
        let row2 = crosschain_transfers::Entity::find_by_id(transfer_id)
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row2.stats_asset_id, Some(asset.id));
    }

    // --- stats_processed (incremental markers) ---

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_processed_seeded_messages_default_zero() {
        let _db = init_db("stats_processed_seeded_messages_default_zero").await;
        fill_mock_interchain_database(&_db).await;
        let interchain_db = InterchainDatabase::new(_db.client());
        let messages = crosschain_messages::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert!(!messages.is_empty());
        for msg in &messages {
            assert_eq!(
                msg.stats_processed, 0,
                "message id={} bridge_id={}",
                msg.id, msg.bridge_id
            );
        }
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_processed_seeded_transfers_default_zero() {
        let _db = init_db("stats_processed_seeded_transfers_default_zero").await;
        fill_mock_interchain_database(&_db).await;
        let interchain_db = InterchainDatabase::new(_db.client());
        let transfers = crosschain_transfers::Entity::find()
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert!(!transfers.is_empty());
        for t in &transfers {
            assert_eq!(t.stats_processed, 0, "transfer id={}", t.id);
        }
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_processed_increment_message_works() {
        let _db = init_db("stats_processed_increment_message_works").await;
        fill_mock_interchain_database(&_db).await;
        let interchain_db = InterchainDatabase::new(_db.client());
        let message_id = 1001i64;
        let bridge_id = 1i32;

        let before = crosschain_messages::Entity::find_by_id((message_id, bridge_id))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(before.stats_processed, 0);

        interchain_db
            .increment_message_stats_processed(message_id, bridge_id)
            .await
            .unwrap();

        let after = crosschain_messages::Entity::find_by_id((message_id, bridge_id))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.stats_processed, 1);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_processed_increment_transfer_works() {
        let _db = init_db("stats_processed_increment_transfer_works").await;
        fill_mock_interchain_database(&_db).await;
        let interchain_db = InterchainDatabase::new(_db.client());
        let transfer_id = 1i64;

        let before = crosschain_transfers::Entity::find_by_id(transfer_id)
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(before.stats_processed, 0);

        interchain_db
            .increment_transfer_stats_processed(transfer_id)
            .await
            .unwrap();

        let after = crosschain_transfers::Entity::find_by_id(transfer_id)
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.stats_processed, 1);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_processed_repeated_increments_increase_value() {
        let _db = init_db("stats_processed_repeated_increments_increase_value").await;
        fill_mock_interchain_database(&_db).await;
        let interchain_db = InterchainDatabase::new(_db.client());
        let message_id = 1002i64;
        let bridge_id = 1i32;
        let transfer_id = 2i64;

        for _ in 0..3 {
            interchain_db
                .increment_message_stats_processed(message_id, bridge_id)
                .await
                .unwrap();
        }
        for _ in 0..5 {
            interchain_db
                .increment_transfer_stats_processed(transfer_id)
                .await
                .unwrap();
        }

        let msg = crosschain_messages::Entity::find_by_id((message_id, bridge_id))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(msg.stats_processed, 3);

        let t = crosschain_transfers::Entity::find_by_id(transfer_id)
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(t.stats_processed, 5);
    }

    // --- stats projection (inline + backfill shared path) ---

    async fn seed_minimal_bridge(db: &sea_orm::DatabaseConnection) {
        chains::Entity::insert_many([
            chains::ActiveModel {
                id: Set(1),
                name: Set("A".into()),
                ..Default::default()
            },
            chains::ActiveModel {
                id: Set(100),
                name: Set("B".into()),
                ..Default::default()
            },
        ])
        .exec(db)
        .await
        .unwrap();
        interchain_indexer_entity::bridges::Entity::insert(
            interchain_indexer_entity::bridges::ActiveModel {
                id: Set(1),
                name: Set("Br".into()),
                ..Default::default()
            },
        )
        .exec(db)
        .await
        .unwrap();
    }

    fn completed_message(id: i64, src: i64, dst: i64) -> crosschain_messages::ActiveModel {
        crosschain_messages::ActiveModel {
            id: Set(id),
            bridge_id: Set(1),
            status: Set(MessageStatus::Completed),
            init_timestamp: Set(Utc::now().naive_utc()),
            src_chain_id: Set(src),
            dst_chain_id: Set(Some(dst)),
            src_tx_hash: Set(Some(vec![0xabu8; 32])),
            stats_processed: Set(0),
            ..Default::default()
        }
    }

    fn completed_message_at(
        id: i64,
        src: i64,
        dst: i64,
        init_timestamp: chrono::NaiveDateTime,
    ) -> crosschain_messages::ActiveModel {
        crosschain_messages::ActiveModel {
            init_timestamp: Set(init_timestamp),
            ..completed_message(id, src, dst)
        }
    }

    fn completed_message_without_indexed_source(
        id: i64,
        src: i64,
        dst: i64,
    ) -> crosschain_messages::ActiveModel {
        crosschain_messages::ActiveModel {
            src_tx_hash: Set(None),
            ..completed_message(id, src, dst)
        }
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_message_updates_stats_messages() {
        let _db = init_db("stats_projection_message_updates_stats_messages").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        crosschain_messages::Entity::insert(completed_message(92001, 1, 100))
            .exec(db)
            .await
            .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(tx, &[(92001i64, 1i32)])
                    .await
                    .map(|_| ())
            })
        })
        .await
        .unwrap();

        let row = stats_messages::Entity::find_by_id((1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.messages_count, 1);
        let m = crosschain_messages::Entity::find_by_id((92001i64, 1i32))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(m.stats_processed, 1);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_message_updates_stats_messages_days() {
        let _db = init_db("stats_projection_message_updates_stats_messages_days").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let day = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        crosschain_messages::Entity::insert(completed_message_at(
            92060,
            1,
            100,
            day.and_hms_opt(12, 34, 56).unwrap(),
        ))
        .exec(db)
        .await
        .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(tx, &[(92060i64, 1i32)])
                    .await
                    .map(|_| ())
            })
        })
        .await
        .unwrap();

        let all_time = stats_messages::Entity::find_by_id((1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(all_time.messages_count, 1);

        let daily = stats_messages_days::Entity::find_by_id((day, 1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(daily.messages_count, 1);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_message_idempotent() {
        let _db = init_db("stats_projection_message_idempotent").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        crosschain_messages::Entity::insert(completed_message(92002, 1, 100))
            .exec(db)
            .await
            .unwrap();

        for _ in 0..2 {
            db.transaction(|tx| {
                Box::pin(async move {
                    crate::stats::projection::project_messages_batch(tx, &[(92002i64, 1i32)])
                        .await
                        .map(|_| ())
                })
            })
            .await
            .unwrap();
        }
        let row = stats_messages::Entity::find_by_id((1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.messages_count, 1);
        let daily_rows = stats_messages_days::Entity::find().all(db).await.unwrap();
        assert_eq!(daily_rows.len(), 1);
        assert_eq!(daily_rows[0].messages_count, 1);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_same_edge_same_day_increments_single_daily_row() {
        let _db = init_db("stats_projection_same_edge_same_day").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let day = NaiveDate::from_ymd_opt(2026, 3, 2).unwrap();

        for (id, hour) in [(92061i64, 1u32), (92062i64, 23u32)] {
            crosschain_messages::Entity::insert(completed_message_at(
                id,
                1,
                100,
                day.and_hms_opt(hour, 0, 0).unwrap(),
            ))
            .exec(db)
            .await
            .unwrap();
        }

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(
                    tx,
                    &[(92061i64, 1i32), (92062i64, 1i32)],
                )
                .await
                .map(|_| ())
            })
        })
        .await
        .unwrap();

        let daily = stats_messages_days::Entity::find()
            .filter(stats_messages_days::Column::Date.eq(day))
            .filter(stats_messages_days::Column::SrcChainId.eq(1i64))
            .filter(stats_messages_days::Column::DstChainId.eq(100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(daily.messages_count, 2);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_same_edge_different_days_create_separate_daily_rows() {
        let _db = init_db("stats_projection_same_edge_different_days").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let day1 = NaiveDate::from_ymd_opt(2026, 3, 3).unwrap();
        let day2 = NaiveDate::from_ymd_opt(2026, 3, 4).unwrap();

        crosschain_messages::Entity::insert_many([
            completed_message_at(92063, 1, 100, day1.and_hms_opt(1, 0, 0).unwrap()),
            completed_message_at(92064, 1, 100, day2.and_hms_opt(1, 0, 0).unwrap()),
        ])
        .exec(db)
        .await
        .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(
                    tx,
                    &[(92063i64, 1i32), (92064i64, 1i32)],
                )
                .await
                .map(|_| ())
            })
        })
        .await
        .unwrap();

        let rows = stats_messages_days::Entity::find()
            .filter(stats_messages_days::Column::SrcChainId.eq(1i64))
            .filter(stats_messages_days::Column::DstChainId.eq(100i64))
            .order_by_asc(stats_messages_days::Column::Date)
            .all(db)
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].date, day1);
        assert_eq!(rows[0].messages_count, 1);
        assert_eq!(rows[1].date, day2);
        assert_eq!(rows[1].messages_count, 1);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_reversed_direction_separate_daily_rows() {
        let _db = init_db("stats_projection_reversed_direction_daily").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let day = NaiveDate::from_ymd_opt(2026, 3, 5).unwrap();

        crosschain_messages::Entity::insert_many([
            completed_message_at(92065, 1, 100, day.and_hms_opt(8, 0, 0).unwrap()),
            completed_message_at(92066, 100, 1, day.and_hms_opt(9, 0, 0).unwrap()),
        ])
        .exec(db)
        .await
        .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(
                    tx,
                    &[(92065i64, 1i32), (92066i64, 1i32)],
                )
                .await
                .map(|_| ())
            })
        })
        .await
        .unwrap();

        let forward = stats_messages_days::Entity::find_by_id((day, 1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        let reverse = stats_messages_days::Entity::find_by_id((day, 100i64, 1i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(forward.messages_count, 1);
        assert_eq!(reverse.messages_count, 1);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_excluded_rows_still_excluded_from_daily_and_all_time() {
        let _db = init_db("stats_projection_excluded_rows").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;

        crosschain_messages::Entity::insert_many([
            crosschain_messages::ActiveModel {
                id: Set(92067),
                bridge_id: Set(1),
                status: Set(MessageStatus::Completed),
                init_timestamp: Set(Utc::now().naive_utc()),
                src_chain_id: Set(1),
                dst_chain_id: Set(None),
                src_tx_hash: Set(Some(vec![0xabu8; 32])),
                stats_processed: Set(0),
                ..Default::default()
            },
            crosschain_messages::ActiveModel {
                id: Set(92068),
                bridge_id: Set(1),
                status: Set(MessageStatus::Failed),
                init_timestamp: Set(Utc::now().naive_utc()),
                src_chain_id: Set(1),
                dst_chain_id: Set(Some(100)),
                src_tx_hash: Set(Some(vec![0xabu8; 32])),
                stats_processed: Set(0),
                ..Default::default()
            },
            crosschain_messages::ActiveModel {
                id: Set(92069),
                bridge_id: Set(1),
                status: Set(MessageStatus::Completed),
                init_timestamp: Set(Utc::now().naive_utc()),
                src_chain_id: Set(1),
                dst_chain_id: Set(Some(100)),
                src_tx_hash: Set(Some(vec![0xabu8; 32])),
                stats_processed: Set(1),
                ..Default::default()
            },
        ])
        .exec(db)
        .await
        .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(
                    tx,
                    &[(92067i64, 1i32), (92068i64, 1i32), (92069i64, 1i32)],
                )
                .await
                .map(|_| ())
            })
        })
        .await
        .unwrap();

        assert_eq!(stats_messages::Entity::find().count(db).await.unwrap(), 0);
        assert_eq!(
            stats_messages_days::Entity::find().count(db).await.unwrap(),
            0
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_messages_days_chain_delete_cascades() {
        let _db = init_db("stats_messages_days_chain_delete_cascades").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("C1".to_string()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("C2".to_string()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("C3".to_string()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();
        let day = NaiveDate::from_ymd_opt(2026, 3, 6).unwrap();

        stats_messages_days::Entity::insert_many([
            stats_messages_days::ActiveModel {
                date: Set(day),
                src_chain_id: Set(1),
                dst_chain_id: Set(2),
                messages_count: Set(1),
                ..Default::default()
            },
            stats_messages_days::ActiveModel {
                date: Set(day),
                src_chain_id: Set(2),
                dst_chain_id: Set(1),
                messages_count: Set(1),
                ..Default::default()
            },
            stats_messages_days::ActiveModel {
                date: Set(day),
                src_chain_id: Set(1),
                dst_chain_id: Set(3),
                messages_count: Set(1),
                ..Default::default()
            },
        ])
        .exec(interchain_db.db.as_ref())
        .await
        .unwrap();

        chains::Entity::delete_by_id(2)
            .exec(interchain_db.db.as_ref())
            .await
            .unwrap();

        assert!(
            stats_messages_days::Entity::find_by_id((day, 1i64, 2i64))
                .one(interchain_db.db.as_ref())
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            stats_messages_days::Entity::find_by_id((day, 2i64, 1i64))
                .one(interchain_db.db.as_ref())
                .await
                .unwrap()
                .is_none()
        );
        let survivor = stats_messages_days::Entity::find_by_id((day, 1i64, 3i64))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(survivor.messages_count, 1);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_transfer_updates_asset_stats() {
        let _db = init_db("stats_projection_transfer_updates_asset_stats").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        crosschain_messages::Entity::insert(completed_message(92003, 1, 100))
            .exec(db)
            .await
            .unwrap();
        let addr_a = [0x11u8; 20].to_vec();
        let addr_b = [0x22u8; 20].to_vec();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: Set(92003),
            message_id: Set(92003),
            bridge_id: Set(1),
            index: Set(0),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(BigDecimal::from(5_000u64)),
            dst_amount: Set(BigDecimal::from(5_000u64)),
            token_src_address: Set(addr_a.clone()),
            token_dst_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(tx, &[(92003i64, 1i32)]).await?;
                crate::stats::projection::project_transfers_batch(tx, &[92003i64]).await?;
                Ok::<(), sea_orm::DbErr>(())
            })
        })
        .await
        .unwrap();

        let t = crosschain_transfers::Entity::find_by_id(92003i64)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(t.stats_processed, 1);
        assert!(t.stats_asset_id.is_some());
        let aid = t.stats_asset_id.unwrap();
        let edge = stats_asset_edges::Entity::find_by_id((aid, 1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.transfers_count, 1);
        assert_eq!(edge.cumulative_amount, BigDecimal::from(5_000u64));
        assert_eq!(edge.amount_side, EdgeAmountSide::Source);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_edge_uses_source_when_source_indexed_even_without_source_decimals() {
        let _db = init_db(
            "stats_projection_edge_uses_source_when_source_indexed_even_without_source_decimals",
        )
        .await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let addr_a = [0xa1u8; 20].to_vec();
        let addr_b = [0xb1u8; 20].to_vec();
        tokens::Entity::insert(tokens::ActiveModel {
            chain_id: Set(100),
            address: Set(addr_b.clone()),
            decimals: Set(Some(8)),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        crosschain_messages::Entity::insert(completed_message(92020, 1, 100))
            .exec(db)
            .await
            .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: Set(92020),
            message_id: Set(92020),
            bridge_id: Set(1),
            index: Set(0),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(BigDecimal::from(100u64)),
            dst_amount: Set(BigDecimal::from(50u64)),
            token_src_address: Set(addr_a.clone()),
            token_dst_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(tx, &[(92020i64, 1i32)]).await?;
                crate::stats::projection::project_transfers_batch(tx, &[92020i64]).await?;
                Ok::<(), sea_orm::DbErr>(())
            })
        })
        .await
        .unwrap();

        let t = crosschain_transfers::Entity::find_by_id(92020i64)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        let aid = t.stats_asset_id.unwrap();
        let edge = stats_asset_edges::Entity::find_by_id((aid, 1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.amount_side, EdgeAmountSide::Source);
        assert_eq!(edge.decimals, None);
        assert_eq!(edge.cumulative_amount, BigDecimal::from(100u64));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_edge_uses_source_when_source_decimals_known() {
        let _db = init_db("stats_projection_edge_uses_source_when_source_decimals_known").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let addr_a = [0xa2u8; 20].to_vec();
        let addr_b = [0xb2u8; 20].to_vec();
        tokens::Entity::insert(tokens::ActiveModel {
            chain_id: Set(1),
            address: Set(addr_a.clone()),
            decimals: Set(Some(6)),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        crosschain_messages::Entity::insert(completed_message(92019, 1, 100))
            .exec(db)
            .await
            .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: Set(92019),
            message_id: Set(92019),
            bridge_id: Set(1),
            index: Set(0),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(BigDecimal::from(100u64)),
            dst_amount: Set(BigDecimal::from(200u64)),
            token_src_address: Set(addr_a.clone()),
            token_dst_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(tx, &[(92019i64, 1i32)]).await?;
                crate::stats::projection::project_transfers_batch(tx, &[92019i64]).await?;
                Ok::<(), sea_orm::DbErr>(())
            })
        })
        .await
        .unwrap();

        let t = crosschain_transfers::Entity::find_by_id(92019i64)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        let aid = t.stats_asset_id.unwrap();
        let edge = stats_asset_edges::Entity::find_by_id((aid, 1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.amount_side, EdgeAmountSide::Source);
        assert_eq!(edge.decimals, Some(6));
        assert_eq!(edge.cumulative_amount, BigDecimal::from(100u64));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_edge_amount_side_sticky_uses_source_amounts_when_source_indexed() {
        let _db = init_db("stats_projection_edge_amount_side_sticky_uses_source_amounts").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let addr_a = [0xc1u8; 20].to_vec();
        let addr_b = [0xc2u8; 20].to_vec();

        crosschain_messages::Entity::insert(completed_message(92021, 1, 100))
            .exec(db)
            .await
            .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: Set(92021),
            message_id: Set(92021),
            bridge_id: Set(1),
            index: Set(0),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(BigDecimal::from(999u64)),
            dst_amount: Set(BigDecimal::from(10u64)),
            token_src_address: Set(addr_a.clone()),
            token_dst_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(tx, &[(92021i64, 1i32)]).await?;
                crate::stats::projection::project_transfers_batch(tx, &[92021i64]).await?;
                Ok::<(), sea_orm::DbErr>(())
            })
        })
        .await
        .unwrap();

        tokens::Entity::insert(tokens::ActiveModel {
            chain_id: Set(1),
            address: Set(addr_a.clone()),
            decimals: Set(Some(18)),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        crosschain_messages::Entity::insert(completed_message(92022, 1, 100))
            .exec(db)
            .await
            .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: Set(92022),
            message_id: Set(92022),
            bridge_id: Set(1),
            index: Set(0),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(BigDecimal::from(888u64)),
            dst_amount: Set(BigDecimal::from(7u64)),
            token_src_address: Set(addr_a.clone()),
            token_dst_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(tx, &[(92022i64, 1i32)]).await?;
                crate::stats::projection::project_transfers_batch(tx, &[92022i64]).await?;
                Ok::<(), sea_orm::DbErr>(())
            })
        })
        .await
        .unwrap();

        let t = crosschain_transfers::Entity::find_by_id(92021i64)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        let aid = t.stats_asset_id.unwrap();
        let edge = stats_asset_edges::Entity::find_by_id((aid, 1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.amount_side, EdgeAmountSide::Source);
        assert_eq!(edge.cumulative_amount, BigDecimal::from(1887u64));
        assert_eq!(edge.decimals, Some(18));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_edge_uses_destination_when_source_chain_not_indexed() {
        let _db =
            init_db("stats_projection_edge_uses_destination_when_source_chain_not_indexed").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let addr_a = [0xd1u8; 20].to_vec();
        let addr_b = [0xd2u8; 20].to_vec();
        tokens::Entity::insert(tokens::ActiveModel {
            chain_id: Set(100),
            address: Set(addr_b.clone()),
            decimals: Set(Some(8)),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        crosschain_messages::Entity::insert(completed_message_without_indexed_source(
            92032, 1, 100,
        ))
        .exec(db)
        .await
        .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: Set(92032),
            message_id: Set(92032),
            bridge_id: Set(1),
            index: Set(0),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(BigDecimal::from(100u64)),
            dst_amount: Set(BigDecimal::from(50u64)),
            token_src_address: Set(addr_a.clone()),
            token_dst_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(tx, &[(92032i64, 1i32)]).await?;
                crate::stats::projection::project_transfers_batch(tx, &[92032i64]).await?;
                Ok::<(), sea_orm::DbErr>(())
            })
        })
        .await
        .unwrap();

        let t = crosschain_transfers::Entity::find_by_id(92032i64)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        let aid = t.stats_asset_id.unwrap();
        let edge = stats_asset_edges::Entity::find_by_id((aid, 1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.amount_side, EdgeAmountSide::Destination);
        assert_eq!(edge.decimals, Some(8));
        assert_eq!(edge.cumulative_amount, BigDecimal::from(50u64));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_batch_two_transfers_same_edge_one_call() {
        let _db = init_db("stats_projection_batch_two_transfers_same_edge").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let addr_a = [0xf1u8; 20].to_vec();
        let addr_b = [0xf2u8; 20].to_vec();

        for (mid, tid, src_amt, dst_amt) in [
            (92030i64, 92030i64, 999u64, 10u64),
            (92031i64, 92031i64, 888u64, 7u64),
        ] {
            crosschain_messages::Entity::insert(completed_message(mid, 1, 100))
                .exec(db)
                .await
                .unwrap();
            crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
                id: Set(tid),
                message_id: Set(mid),
                bridge_id: Set(1),
                index: Set(0),
                token_src_chain_id: Set(1),
                token_dst_chain_id: Set(100),
                src_amount: Set(BigDecimal::from(src_amt)),
                dst_amount: Set(BigDecimal::from(dst_amt)),
                token_src_address: Set(addr_a.clone()),
                token_dst_address: Set(addr_b.clone()),
                ..Default::default()
            })
            .exec(db)
            .await
            .unwrap();
        }

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(
                    tx,
                    &[(92030i64, 1i32), (92031i64, 1i32)],
                )
                .await?;
                crate::stats::projection::project_transfers_batch(tx, &[92030i64, 92031i64])
                    .await?;
                Ok::<(), sea_orm::DbErr>(())
            })
        })
        .await
        .unwrap();

        let t1 = crosschain_transfers::Entity::find_by_id(92030i64)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        let t2 = crosschain_transfers::Entity::find_by_id(92031i64)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(t1.stats_processed, 1);
        assert_eq!(t2.stats_processed, 1);
        let aid = t1.stats_asset_id.unwrap();
        assert_eq!(t2.stats_asset_id, Some(aid));
        let edge = stats_asset_edges::Entity::find_by_id((aid, 1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.amount_side, EdgeAmountSide::Source);
        assert_eq!(edge.transfers_count, 2);
        assert_eq!(edge.cumulative_amount, BigDecimal::from(1887u64));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_rejects_conflicting_edge_decimals() {
        let _db = init_db("stats_projection_rejects_conflicting_edge_decimals").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let addr_a = [0xd1u8; 20].to_vec();
        let addr_b = [0xd2u8; 20].to_vec();

        let aid = stats_assets::Entity::insert(stats_assets::ActiveModel {
            ..Default::default()
        })
        .exec_with_returning(db)
        .await
        .unwrap()
        .id;
        stats_asset_tokens::Entity::insert(stats_asset_tokens::ActiveModel {
            stats_asset_id: Set(aid),
            chain_id: Set(1),
            token_address: Set(addr_a.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();
        stats_asset_tokens::Entity::insert(stats_asset_tokens::ActiveModel {
            stats_asset_id: Set(aid),
            chain_id: Set(100),
            token_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        tokens::Entity::insert(tokens::ActiveModel {
            chain_id: Set(1),
            address: Set(addr_a.clone()),
            decimals: Set(Some(18)),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        stats_asset_edges::Entity::insert(stats_asset_edges::ActiveModel {
            stats_asset_id: Set(aid),
            src_chain_id: Set(1),
            dst_chain_id: Set(100),
            transfers_count: Set(0),
            cumulative_amount: Set(BigDecimal::from(0u64)),
            decimals: Set(Some(17)),
            amount_side: Set(EdgeAmountSide::Source),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        crosschain_messages::Entity::insert(completed_message(92023, 1, 100))
            .exec(db)
            .await
            .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: Set(92023),
            message_id: Set(92023),
            bridge_id: Set(1),
            index: Set(0),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(BigDecimal::from(1u64)),
            dst_amount: Set(BigDecimal::from(1u64)),
            token_src_address: Set(addr_a.clone()),
            token_dst_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        let res = db
            .transaction(|tx| {
                Box::pin(async move {
                    crate::stats::projection::project_messages_batch(tx, &[(92023i64, 1i32)])
                        .await?;
                    crate::stats::projection::project_transfers_batch(tx, &[92023i64]).await?;
                    Ok::<(), sea_orm::DbErr>(())
                })
            })
            .await;
        assert!(res.is_err(), "expected decimals conflict");
        let t = crosschain_transfers::Entity::find_by_id(92023i64)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(t.stats_processed, 0);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_rejects_conflicting_asset_mappings() {
        let _db = init_db("stats_projection_rejects_conflicting_asset_mappings").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let addr_a = [0xe1u8; 20].to_vec();
        let addr_b = [0xe2u8; 20].to_vec();

        let aid_a = stats_assets::Entity::insert(stats_assets::ActiveModel {
            ..Default::default()
        })
        .exec_with_returning(db)
        .await
        .unwrap()
        .id;
        let aid_b = stats_assets::Entity::insert(stats_assets::ActiveModel {
            ..Default::default()
        })
        .exec_with_returning(db)
        .await
        .unwrap()
        .id;
        stats_asset_tokens::Entity::insert(stats_asset_tokens::ActiveModel {
            stats_asset_id: Set(aid_a),
            chain_id: Set(1),
            token_address: Set(addr_a.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();
        stats_asset_tokens::Entity::insert(stats_asset_tokens::ActiveModel {
            stats_asset_id: Set(aid_b),
            chain_id: Set(100),
            token_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        crosschain_messages::Entity::insert(completed_message(92024, 1, 100))
            .exec(db)
            .await
            .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: Set(92024),
            message_id: Set(92024),
            bridge_id: Set(1),
            index: Set(0),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(BigDecimal::from(1u64)),
            dst_amount: Set(BigDecimal::from(1u64)),
            token_src_address: Set(addr_a.clone()),
            token_dst_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        let res = db
            .transaction(|tx| {
                Box::pin(async move {
                    crate::stats::projection::project_messages_batch(tx, &[(92024i64, 1i32)])
                        .await?;
                    crate::stats::projection::project_transfers_batch(tx, &[92024i64]).await?;
                    Ok::<(), sea_orm::DbErr>(())
                })
            })
            .await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("different stats assets") || msg.contains("92024"),
            "unexpected error: {msg}"
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_enrichment_projection_fills_stats_asset_metadata_from_tokens() {
        let _db = init_db("stats_enrichment_projection_metadata").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let addr_a = [0x71u8; 20].to_vec();
        let addr_b = [0x72u8; 20].to_vec();

        tokens::Entity::insert(tokens::ActiveModel {
            chain_id: Set(1),
            address: Set(addr_a.clone()),
            name: Set(Some("SrcGold".to_string())),
            symbol: Set(Some("SGOLD".to_string())),
            token_icon: Set(Some("https://src/icon.png".to_string())),
            decimals: Set(Some(9)),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();
        tokens::Entity::insert(tokens::ActiveModel {
            chain_id: Set(100),
            address: Set(addr_b.clone()),
            name: Set(Some("OnlyDst".to_string())),
            symbol: Set(Some("ODST".to_string())),
            decimals: Set(Some(8)),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        crosschain_messages::Entity::insert(completed_message(92050, 1, 100))
            .exec(db)
            .await
            .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: Set(92050),
            message_id: Set(92050),
            bridge_id: Set(1),
            index: Set(0),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(BigDecimal::from(1u64)),
            dst_amount: Set(BigDecimal::from(1u64)),
            token_src_address: Set(addr_a.clone()),
            token_dst_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(tx, &[(92050i64, 1i32)]).await?;
                crate::stats::projection::project_transfers_batch(tx, &[92050i64]).await?;
                Ok::<(), sea_orm::DbErr>(())
            })
        })
        .await
        .unwrap();

        let t = crosschain_transfers::Entity::find_by_id(92050i64)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(t.stats_processed, 1);
        let aid = t.stats_asset_id.unwrap();
        let asset = stats_assets::Entity::find_by_id(aid)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(asset.name.as_deref(), Some("SrcGold"));
        assert_eq!(asset.symbol.as_deref(), Some("SGOLD"));
        assert_eq!(asset.icon_url.as_deref(), Some("https://src/icon.png"));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_enrichment_projection_succeeds_without_token_rows() {
        let _db = init_db("stats_enrichment_no_tokens").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        crosschain_messages::Entity::insert(completed_message(92051, 1, 100))
            .exec(db)
            .await
            .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: Set(92051),
            message_id: Set(92051),
            bridge_id: Set(1),
            index: Set(0),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(BigDecimal::from(3u64)),
            dst_amount: Set(BigDecimal::from(3u64)),
            token_src_address: Set([0x81u8; 20].to_vec()),
            token_dst_address: Set([0x82u8; 20].to_vec()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(tx, &[(92051i64, 1i32)]).await?;
                crate::stats::projection::project_transfers_batch(tx, &[92051i64]).await?;
                Ok::<(), sea_orm::DbErr>(())
            })
        })
        .await
        .unwrap();

        let t = crosschain_transfers::Entity::find_by_id(92051i64)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(t.stats_processed, 1);
        let asset = stats_assets::Entity::find_by_id(t.stats_asset_id.unwrap())
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert!(asset.name.as_ref().is_none_or(|s| s.is_empty()));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_enrichment_propagate_upsert_fills_asset_and_edge_decimals() {
        let _db = init_db("stats_enrichment_propagate_edge").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let ic = InterchainDatabase::new(_db.client());
        let addr_b = [0x91u8; 20].to_vec();

        let aid = stats_assets::Entity::insert(stats_assets::ActiveModel {
            ..Default::default()
        })
        .exec_with_returning(db)
        .await
        .unwrap()
        .id;
        stats_asset_tokens::Entity::insert(stats_asset_tokens::ActiveModel {
            stats_asset_id: Set(aid),
            chain_id: Set(100),
            token_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        stats_asset_edges::Entity::insert(stats_asset_edges::ActiveModel {
            stats_asset_id: Set(aid),
            src_chain_id: Set(1),
            dst_chain_id: Set(100),
            transfers_count: Set(0),
            cumulative_amount: Set(BigDecimal::from(0u64)),
            decimals: Set(None),
            amount_side: Set(EdgeAmountSide::Destination),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        ic.upsert_token_info(tokens::ActiveModel {
            chain_id: Set(100),
            address: Set(addr_b.clone()),
            name: Set(Some("Bridged".to_string())),
            symbol: Set(Some("BRG".to_string())),
            decimals: Set(Some(12)),
            token_icon: Set(None),
            ..Default::default()
        })
        .await
        .unwrap();

        let token = tokens::Entity::find()
            .filter(tokens::Column::ChainId.eq(100i64))
            .filter(tokens::Column::Address.eq(addr_b.clone()))
            .one(db)
            .await
            .unwrap()
            .unwrap();

        ic.propagate_token_info_to_stats_tables(100, &addr_b, &token)
            .await
            .unwrap();

        let asset = stats_assets::Entity::find_by_id(aid)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(asset.name.as_deref(), Some("Bridged"));
        assert_eq!(asset.symbol.as_deref(), Some("BRG"));

        let edge = stats_asset_edges::Entity::find_by_id((aid, 1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.decimals, Some(12));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_enrichment_propagate_skips_unrelated_destination_edge() {
        let _db = init_db("stats_enrichment_unrelated_edge").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        chains::Entity::insert(chains::ActiveModel {
            id: Set(200),
            name: Set("C".into()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();
        let ic = InterchainDatabase::new(_db.client());
        let addr_b = [0xa1u8; 20].to_vec();

        let aid = stats_assets::Entity::insert(stats_assets::ActiveModel {
            ..Default::default()
        })
        .exec_with_returning(db)
        .await
        .unwrap()
        .id;
        stats_asset_tokens::Entity::insert(stats_asset_tokens::ActiveModel {
            stats_asset_id: Set(aid),
            chain_id: Set(100),
            token_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        stats_asset_edges::Entity::insert(stats_asset_edges::ActiveModel {
            stats_asset_id: Set(aid),
            src_chain_id: Set(1),
            dst_chain_id: Set(200i64),
            transfers_count: Set(0),
            cumulative_amount: Set(BigDecimal::from(0u64)),
            decimals: Set(None),
            amount_side: Set(EdgeAmountSide::Destination),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        ic.upsert_token_info(tokens::ActiveModel {
            chain_id: Set(100),
            address: Set(addr_b.clone()),
            name: Set(Some("T".to_string())),
            symbol: Set(Some("T".to_string())),
            decimals: Set(Some(7)),
            token_icon: Set(None),
            ..Default::default()
        })
        .await
        .unwrap();

        let token = tokens::Entity::find()
            .filter(tokens::Column::ChainId.eq(100i64))
            .filter(tokens::Column::Address.eq(addr_b.clone()))
            .one(db)
            .await
            .unwrap()
            .unwrap();

        ic.propagate_token_info_to_stats_tables(100, &addr_b, &token)
            .await
            .unwrap();

        let edge = stats_asset_edges::Entity::find_by_id((aid, 1i64, 200i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert!(
            edge.decimals.is_none(),
            "dst chain 200 should not take decimals from token on chain 100"
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_enrichment_propagate_does_not_overwrite_conflicting_decimals() {
        let _db = init_db("stats_enrichment_decimal_conflict").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let ic = InterchainDatabase::new(_db.client());
        let addr_b = [0xb1u8; 20].to_vec();

        let aid = stats_assets::Entity::insert(stats_assets::ActiveModel {
            ..Default::default()
        })
        .exec_with_returning(db)
        .await
        .unwrap()
        .id;
        stats_asset_tokens::Entity::insert(stats_asset_tokens::ActiveModel {
            stats_asset_id: Set(aid),
            chain_id: Set(100),
            token_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        stats_asset_edges::Entity::insert(stats_asset_edges::ActiveModel {
            stats_asset_id: Set(aid),
            src_chain_id: Set(1),
            dst_chain_id: Set(100),
            transfers_count: Set(0),
            cumulative_amount: Set(BigDecimal::from(0u64)),
            decimals: Set(Some(5)),
            amount_side: Set(EdgeAmountSide::Destination),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        ic.upsert_token_info(tokens::ActiveModel {
            chain_id: Set(100),
            address: Set(addr_b.clone()),
            name: Set(None),
            symbol: Set(None),
            decimals: Set(Some(11)),
            token_icon: Set(None),
            ..Default::default()
        })
        .await
        .unwrap();

        let token = tokens::Entity::find()
            .filter(tokens::Column::ChainId.eq(100i64))
            .filter(tokens::Column::Address.eq(addr_b.clone()))
            .one(db)
            .await
            .unwrap()
            .unwrap();

        ic.propagate_token_info_to_stats_tables(100, &addr_b, &token)
            .await
            .unwrap();

        let edge = stats_asset_edges::Entity::find_by_id((aid, 1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.decimals, Some(5));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_enrichment_propagate_preserves_non_empty_asset_metadata() {
        let _db = init_db("stats_enrichment_keep_metadata").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        let ic = InterchainDatabase::new(_db.client());
        let addr_b = [0xc1u8; 20].to_vec();

        let aid = stats_assets::Entity::insert(stats_assets::ActiveModel {
            name: Set(Some("ManualName".to_string())),
            symbol: Set(Some("MN".to_string())),
            icon_url: Set(Some("https://keep.ico".to_string())),
            ..Default::default()
        })
        .exec_with_returning(db)
        .await
        .unwrap()
        .id;
        stats_asset_tokens::Entity::insert(stats_asset_tokens::ActiveModel {
            stats_asset_id: Set(aid),
            chain_id: Set(100),
            token_address: Set(addr_b.clone()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        ic.upsert_token_info(tokens::ActiveModel {
            chain_id: Set(100),
            address: Set(addr_b.clone()),
            name: Set(Some("TokenOther".to_string())),
            symbol: Set(Some("TO".to_string())),
            decimals: Set(Some(6)),
            token_icon: Set(Some("https://other.ico".to_string())),
            ..Default::default()
        })
        .await
        .unwrap();

        let token = tokens::Entity::find()
            .filter(tokens::Column::ChainId.eq(100i64))
            .filter(tokens::Column::Address.eq(addr_b.clone()))
            .one(db)
            .await
            .unwrap()
            .unwrap();

        ic.propagate_token_info_to_stats_tables(100, &addr_b, &token)
            .await
            .unwrap();

        let asset = stats_assets::Entity::find_by_id(aid)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(asset.name.as_deref(), Some("ManualName"));
        assert_eq!(asset.symbol.as_deref(), Some("MN"));
        assert_eq!(asset.icon_url.as_deref(), Some("https://keep.ico"));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_transfer_idempotent() {
        let _db = init_db("stats_projection_transfer_idempotent").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        crosschain_messages::Entity::insert(completed_message(92004, 1, 100))
            .exec(db)
            .await
            .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: Set(92004),
            message_id: Set(92004),
            bridge_id: Set(1),
            index: Set(0),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(BigDecimal::from(100u64)),
            dst_amount: Set(BigDecimal::from(100u64)),
            token_src_address: Set([0x33u8; 20].to_vec()),
            token_dst_address: Set([0x44u8; 20].to_vec()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        for _ in 0..2 {
            db.transaction(|tx| {
                Box::pin(async move {
                    crate::stats::projection::project_messages_batch(tx, &[(92004i64, 1i32)])
                        .await?;
                    crate::stats::projection::project_transfers_batch(tx, &[92004i64]).await?;
                    Ok::<(), sea_orm::DbErr>(())
                })
            })
            .await
            .unwrap();
        }
        let t = crosschain_transfers::Entity::find_by_id(92004i64)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        let aid = t.stats_asset_id.unwrap();
        let edge = stats_asset_edges::Entity::find_by_id((aid, 1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.transfers_count, 1);
        assert_eq!(edge.cumulative_amount, BigDecimal::from(100u64));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_backfill_matches_inline_projection() {
        let _db = init_db("stats_backfill_matches_inline_projection").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        crosschain_messages::Entity::insert(completed_message(92005, 1, 100))
            .exec(db)
            .await
            .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: Set(92005),
            message_id: Set(92005),
            bridge_id: Set(1),
            index: Set(0),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(BigDecimal::from(42u64)),
            dst_amount: Set(BigDecimal::from(42u64)),
            token_src_address: Set([0x55u8; 20].to_vec()),
            token_dst_address: Set([0x66u8; 20].to_vec()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        let ic = InterchainDatabase::new(_db.client());
        let r = ic.backfill_stats_projection_round(50, 50).await.unwrap();
        assert!(r.messages_processed >= 1);
        assert!(r.transfers_processed >= 1);

        let m = crosschain_messages::Entity::find_by_id((92005i64, 1i32))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(m.stats_processed, 1);
        let t = crosschain_transfers::Entity::find_by_id(92005i64)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(t.stats_processed, 1);
        assert!(t.stats_asset_id.is_some());

        let r2 = ic.backfill_stats_projection_round(50, 50).await.unwrap();
        assert_eq!(r2.messages_processed, 0);
        assert_eq!(r2.transfers_processed, 0);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_backfill_until_idle_empty_succeeds() {
        let _db = init_db("stats_backfill_until_idle_empty_succeeds").await;
        let ic = InterchainDatabase::new(_db.client());
        ic.backfill_stats_until_idle().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_backfill_until_idle_drains_pending() {
        let _db = init_db("stats_backfill_until_idle_drains_pending").await;
        let ic = InterchainDatabase::new(_db.client());
        let db = ic.db.as_ref();
        seed_minimal_bridge(db).await;
        crosschain_messages::Entity::insert(completed_message(92008, 1, 100))
            .exec(db)
            .await
            .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: Set(92008),
            message_id: Set(92008),
            bridge_id: Set(1),
            index: Set(0),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(BigDecimal::from(7u64)),
            dst_amount: Set(BigDecimal::from(7u64)),
            token_src_address: Set([0x77u8; 20].to_vec()),
            token_dst_address: Set([0x88u8; 20].to_vec()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        ic.backfill_stats_until_idle().await.unwrap();

        let m = crosschain_messages::Entity::find_by_id((92008i64, 1i32))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(m.stats_processed, 1);
        let t = crosschain_transfers::Entity::find_by_id(92008i64)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(t.stats_processed, 1);
        let row = stats_messages::Entity::find_by_id((1i64, 100i64))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.messages_count, 1);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_projection_rollback_does_not_increment_marker() {
        let _db = init_db("stats_projection_rollback_does_not_increment_marker").await;
        let conn = _db.client();
        let db = conn.as_ref();
        seed_minimal_bridge(db).await;
        crosschain_messages::Entity::insert(completed_message(92006, 1, 100))
            .exec(db)
            .await
            .unwrap();

        let res = db
            .transaction(|tx| {
                Box::pin(async move {
                    crate::stats::projection::project_messages_batch(tx, &[(92006i64, 1i32)])
                        .await?;
                    Err::<(), sea_orm::DbErr>(sea_orm::DbErr::Custom("forced abort".into()))
                })
            })
            .await;
        assert!(res.is_err());

        let m = crosschain_messages::Entity::find_by_id((92006i64, 1i32))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(m.stats_processed, 0);
        assert!(
            stats_messages::Entity::find_by_id((1i64, 100i64))
                .one(db)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_asset_delete_cascades() {
        let _db = init_db("stats_asset_delete_cascades").await;
        fill_mock_interchain_database(&_db).await;
        let interchain_db = InterchainDatabase::new(_db.client());
        // mock has chains 1 (Ethereum) and 100 (Gnosis)
        let asset = interchain_db
            .create_stats_asset(Some("Del".to_string()), None, None)
            .await
            .unwrap();
        interchain_db
            .link_token_to_stats_asset(asset.id, 1, vec![0xdd; 20])
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_asset_edge(
                asset.id,
                1,
                100,
                BigDecimal::from(1u64),
                EdgeAmountSide::Source,
                None,
            )
            .await
            .unwrap();
        let transfer_id = 1i64;
        interchain_db
            .assign_transfer_stats_asset(transfer_id, Some(asset.id))
            .await
            .unwrap();

        stats_assets::Entity::delete_by_id(asset.id)
            .exec(interchain_db.db.as_ref())
            .await
            .unwrap();

        assert!(
            stats_asset_tokens::Entity::find()
                .filter(stats_asset_tokens::Column::StatsAssetId.eq(asset.id))
                .one(interchain_db.db.as_ref())
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            stats_asset_edges::Entity::find_by_id((asset.id, 1i64, 100i64))
                .one(interchain_db.db.as_ref())
                .await
                .unwrap()
                .is_none()
        );
        let t = crosschain_transfers::Entity::find_by_id(transfer_id)
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(t.stats_asset_id, None);
    }

    // --- stats_messages: directional chain-to-chain message counts ---

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_messages_insert_first_row() {
        let _db = init_db("stats_messages_insert_first_row").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("C1".to_string()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("C2".to_string()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        interchain_db
            .create_or_update_stats_messages(1, 2, 1)
            .await
            .unwrap();

        let row = interchain_db
            .get_stats_messages_row(1, 2)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.src_chain_id, 1);
        assert_eq!(row.dst_chain_id, 2);
        assert_eq!(row.messages_count, 1);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_messages_upsert_increments_count() {
        let _db = init_db("stats_messages_upsert_increments_count").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(10),
                    name: Set("A".to_string()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(20),
                    name: Set("B".to_string()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        interchain_db
            .create_or_update_stats_messages(10, 20, 1)
            .await
            .unwrap();
        let r1 = interchain_db
            .get_stats_messages_row(10, 20)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r1.messages_count, 1);

        interchain_db
            .create_or_update_stats_messages(10, 20, 1)
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(10, 20, 1)
            .await
            .unwrap();
        let r2 = interchain_db
            .get_stats_messages_row(10, 20)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r2.messages_count, 3);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_messages_reversed_direction_separate_row() {
        let _db = init_db("stats_messages_reversed_direction_separate_row").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(100),
                    name: Set("X".to_string()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(200),
                    name: Set("Y".to_string()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        interchain_db
            .create_or_update_stats_messages(100, 200, 1)
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(200, 100, 1)
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(200, 100, 1)
            .await
            .unwrap();

        let ab = interchain_db
            .get_stats_messages_row(100, 200)
            .await
            .unwrap()
            .unwrap();
        let ba = interchain_db
            .get_stats_messages_row(200, 100)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ab.messages_count, 1);
        assert_eq!(ba.messages_count, 2);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_messages_chain_delete_cascades() {
        let _db = init_db("stats_messages_chain_delete_cascades").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("C1".to_string()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("C2".to_string()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("C3".to_string()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        interchain_db
            .create_or_update_stats_messages(1, 2, 1)
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(2, 1, 1)
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(1, 3, 1)
            .await
            .unwrap();

        chains::Entity::delete_by_id(2)
            .exec(interchain_db.db.as_ref())
            .await
            .unwrap();

        assert!(
            interchain_db
                .get_stats_messages_row(1, 2)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            interchain_db
                .get_stats_messages_row(2, 1)
                .await
                .unwrap()
                .is_none()
        );
        let row_1_3 = interchain_db
            .get_stats_messages_row(1, 3)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row_1_3.messages_count, 1);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn stats_messages_migration_and_db_layer() {
        let _db = init_db("stats_messages_migration_and_db_layer").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("Chain1".to_string()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("Chain2".to_string()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        interchain_db
            .create_or_update_stats_messages(1, 2, 1)
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(1, 2, 1)
            .await
            .unwrap();

        let row = stats_messages::Entity::find_by_id((1i64, 2i64))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.src_chain_id, 1);
        assert_eq!(row.dst_chain_id, 2);
        assert_eq!(row.messages_count, 2);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn message_paths_outgoing_all_time_reads_stats_messages() {
        let _db = init_db("message_paths_outgoing_all_time").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("A".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("B".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("C".into()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(1, 2, 5)
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(1, 3, 2)
            .await
            .unwrap();
        stats_messages_days::Entity::insert(stats_messages_days::ActiveModel {
            date: Set(NaiveDate::from_ymd_opt(2026, 3, 7).unwrap()),
            src_chain_id: Set(1),
            dst_chain_id: Set(2),
            messages_count: Set(999),
            ..Default::default()
        })
        .exec(interchain_db.db.as_ref())
        .await
        .unwrap();

        let rows = interchain_db
            .get_outgoing_message_paths(1, None, None, None, false)
            .await
            .unwrap();
        assert_eq!(
            rows,
            vec![
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 2,
                    messages_count: 5
                },
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 3,
                    messages_count: 2
                },
            ]
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn message_paths_incoming_all_time_reads_stats_messages() {
        let _db = init_db("message_paths_incoming_all_time").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("A".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("B".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("C".into()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(1, 3, 4)
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(2, 3, 6)
            .await
            .unwrap();

        let rows = interchain_db
            .get_incoming_message_paths(3, None, None, None, false)
            .await
            .unwrap();
        assert_eq!(
            rows,
            vec![
                MessagePathStatsRow {
                    src_chain_id: 2,
                    dst_chain_id: 3,
                    messages_count: 6
                },
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 3,
                    messages_count: 4
                },
            ]
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn message_paths_include_zero_outgoing_all_time_expands_known_chains() {
        let _db = init_db("message_paths_include_zero_outgoing_all_time").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("A".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("B".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("C".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(4),
                    name: Set("D".into()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(1, 2, 5)
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(1, 4, 2)
            .await
            .unwrap();

        let rows = interchain_db
            .get_outgoing_message_paths(1, None, None, None, true)
            .await
            .unwrap();
        assert_eq!(
            rows,
            vec![
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 2,
                    messages_count: 5
                },
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 4,
                    messages_count: 2
                },
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 3,
                    messages_count: 0
                },
            ]
        );
        assert!(rows.iter().all(|row| row.dst_chain_id != 1));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn message_paths_include_zero_incoming_all_time_expands_known_chains() {
        let _db = init_db("message_paths_include_zero_incoming_all_time").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("A".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("B".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("C".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(4),
                    name: Set("D".into()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(1, 4, 4)
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(3, 4, 6)
            .await
            .unwrap();

        let rows = interchain_db
            .get_incoming_message_paths(4, None, None, None, true)
            .await
            .unwrap();
        assert_eq!(
            rows,
            vec![
                MessagePathStatsRow {
                    src_chain_id: 3,
                    dst_chain_id: 4,
                    messages_count: 6
                },
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 4,
                    messages_count: 4
                },
                MessagePathStatsRow {
                    src_chain_id: 2,
                    dst_chain_id: 4,
                    messages_count: 0
                },
            ]
        );
        assert!(rows.iter().all(|row| row.src_chain_id != 4));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn message_paths_bounded_queries_sum_daily_rows_and_order_deterministically() {
        let _db = init_db("message_paths_bounded_queries").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("A".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("B".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("C".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(4),
                    name: Set("D".into()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        for (date, src, dst, count) in [
            (NaiveDate::from_ymd_opt(2026, 3, 8).unwrap(), 1, 2, 2),
            (NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(), 1, 2, 3),
            (NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(), 1, 3, 5),
            (NaiveDate::from_ymd_opt(2026, 3, 10).unwrap(), 1, 4, 5),
            (NaiveDate::from_ymd_opt(2026, 3, 11).unwrap(), 2, 1, 4),
            (NaiveDate::from_ymd_opt(2026, 3, 12).unwrap(), 3, 1, 1),
        ] {
            stats_messages_days::Entity::insert(stats_messages_days::ActiveModel {
                date: Set(date),
                src_chain_id: Set(src),
                dst_chain_id: Set(dst),
                messages_count: Set(count),
                ..Default::default()
            })
            .exec(interchain_db.db.as_ref())
            .await
            .unwrap();
        }

        let outgoing = interchain_db
            .get_outgoing_message_paths(
                1,
                Some(NaiveDate::from_ymd_opt(2026, 3, 8).unwrap()),
                Some(NaiveDate::from_ymd_opt(2026, 3, 11).unwrap()),
                None,
                false,
            )
            .await
            .unwrap();
        assert_eq!(
            outgoing,
            vec![
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 2,
                    messages_count: 5
                },
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 3,
                    messages_count: 5
                },
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 4,
                    messages_count: 5
                },
            ]
        );

        let incoming = interchain_db
            .get_incoming_message_paths(
                1,
                Some(NaiveDate::from_ymd_opt(2026, 3, 11).unwrap()),
                Some(NaiveDate::from_ymd_opt(2026, 3, 13).unwrap()),
                None,
                false,
            )
            .await
            .unwrap();
        assert_eq!(
            incoming,
            vec![
                MessagePathStatsRow {
                    src_chain_id: 2,
                    dst_chain_id: 1,
                    messages_count: 4
                },
                MessagePathStatsRow {
                    src_chain_id: 3,
                    dst_chain_id: 1,
                    messages_count: 1
                },
            ]
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn message_paths_bounded_queries_apply_open_and_half_open_ranges() {
        let _db = init_db("message_paths_bounded_ranges").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("A".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("B".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("C".into()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        for (date, src, dst, count) in [
            (NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(), 1, 2, 1),
            (NaiveDate::from_ymd_opt(2026, 3, 2).unwrap(), 1, 2, 2),
            (NaiveDate::from_ymd_opt(2026, 3, 3).unwrap(), 1, 3, 3),
            (NaiveDate::from_ymd_opt(2026, 3, 4).unwrap(), 2, 1, 4),
        ] {
            stats_messages_days::Entity::insert(stats_messages_days::ActiveModel {
                date: Set(date),
                src_chain_id: Set(src),
                dst_chain_id: Set(dst),
                messages_count: Set(count),
                ..Default::default()
            })
            .exec(interchain_db.db.as_ref())
            .await
            .unwrap();
        }

        let from_only = interchain_db
            .get_outgoing_message_paths(
                1,
                Some(NaiveDate::from_ymd_opt(2026, 3, 2).unwrap()),
                None,
                None,
                false,
            )
            .await
            .unwrap();
        assert_eq!(
            from_only,
            vec![
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 3,
                    messages_count: 3
                },
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 2,
                    messages_count: 2
                },
            ]
        );

        let to_only = interchain_db
            .get_outgoing_message_paths(
                1,
                None,
                Some(NaiveDate::from_ymd_opt(2026, 3, 3).unwrap()),
                None,
                false,
            )
            .await
            .unwrap();
        assert_eq!(
            to_only,
            vec![MessagePathStatsRow {
                src_chain_id: 1,
                dst_chain_id: 2,
                messages_count: 3
            }]
        );

        let half_open = interchain_db
            .get_outgoing_message_paths(
                1,
                Some(NaiveDate::from_ymd_opt(2026, 3, 2).unwrap()),
                Some(NaiveDate::from_ymd_opt(2026, 3, 3).unwrap()),
                None,
                false,
            )
            .await
            .unwrap();
        assert_eq!(
            half_open,
            vec![MessagePathStatsRow {
                src_chain_id: 1,
                dst_chain_id: 2,
                messages_count: 2
            }]
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn message_paths_include_zero_bounded_queries_expand_known_chains() {
        let _db = init_db("message_paths_include_zero_bounded").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("A".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("B".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("C".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(4),
                    name: Set("D".into()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        for (date, src, dst, count) in [
            (NaiveDate::from_ymd_opt(2026, 3, 8).unwrap(), 1, 2, 2),
            (NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(), 1, 2, 3),
            (NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(), 1, 4, 1),
        ] {
            stats_messages_days::Entity::insert(stats_messages_days::ActiveModel {
                date: Set(date),
                src_chain_id: Set(src),
                dst_chain_id: Set(dst),
                messages_count: Set(count),
                ..Default::default()
            })
            .exec(interchain_db.db.as_ref())
            .await
            .unwrap();
        }

        let rows = interchain_db
            .get_outgoing_message_paths(
                1,
                Some(NaiveDate::from_ymd_opt(2026, 3, 8).unwrap()),
                Some(NaiveDate::from_ymd_opt(2026, 3, 10).unwrap()),
                None,
                true,
            )
            .await
            .unwrap();
        assert_eq!(
            rows,
            vec![
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 2,
                    messages_count: 5
                },
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 4,
                    messages_count: 1
                },
                MessagePathStatsRow {
                    src_chain_id: 1,
                    dst_chain_id: 3,
                    messages_count: 0
                },
            ]
        );
        assert!(rows.iter().all(|row| row.dst_chain_id != 1));
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn message_paths_invalid_or_empty_range_returns_empty() {
        let _db = init_db("message_paths_invalid_range").await;
        let interchain_db = InterchainDatabase::new(_db.client());

        assert!(
            interchain_db
                .get_outgoing_message_paths(
                    1,
                    Some(NaiveDate::from_ymd_opt(2026, 3, 2).unwrap()),
                    Some(NaiveDate::from_ymd_opt(2026, 3, 2).unwrap()),
                    None,
                    true,
                )
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            interchain_db
                .get_incoming_message_paths(
                    1,
                    Some(NaiveDate::from_ymd_opt(2026, 3, 3).unwrap()),
                    Some(NaiveDate::from_ymd_opt(2026, 3, 2).unwrap()),
                    None,
                    true,
                )
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn message_paths_outgoing_counterparty_filters_destinations() {
        let _db = init_db("message_paths_outgoing_counterparty").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("A".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("B".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("C".into()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(1, 2, 5)
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(1, 3, 2)
            .await
            .unwrap();

        let rows = interchain_db
            .get_outgoing_message_paths(1, None, None, Some(&[3]), true)
            .await
            .unwrap();
        assert_eq!(
            rows,
            vec![MessagePathStatsRow {
                src_chain_id: 1,
                dst_chain_id: 3,
                messages_count: 2
            }]
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn message_paths_include_zero_counterparty_does_not_synthesize_missing_rows() {
        let _db = init_db("message_paths_include_zero_counterparty_missing").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("A".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("B".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("C".into()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(1, 2, 5)
            .await
            .unwrap();

        let rows = interchain_db
            .get_outgoing_message_paths(1, None, None, Some(&[3]), true)
            .await
            .unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn message_paths_incoming_counterparty_filters_sources() {
        let _db = init_db("message_paths_incoming_counterparty").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("A".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("B".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("C".into()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(1, 3, 4)
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(2, 3, 6)
            .await
            .unwrap();

        let rows = interchain_db
            .get_incoming_message_paths(3, None, None, Some(&[1]), true)
            .await
            .unwrap();
        assert_eq!(
            rows,
            vec![MessagePathStatsRow {
                src_chain_id: 1,
                dst_chain_id: 3,
                messages_count: 4
            }]
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn message_paths_bounded_counterparty_filter_applies_in_sql() {
        let _db = init_db("message_paths_bounded_counterparty").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("A".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("B".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("C".into()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        for (date, src, dst, count) in [
            (NaiveDate::from_ymd_opt(2026, 3, 8).unwrap(), 1, 2, 2),
            (NaiveDate::from_ymd_opt(2026, 3, 8).unwrap(), 1, 3, 7),
        ] {
            stats_messages_days::Entity::insert(stats_messages_days::ActiveModel {
                date: Set(date),
                src_chain_id: Set(src),
                dst_chain_id: Set(dst),
                messages_count: Set(count),
                ..Default::default()
            })
            .exec(interchain_db.db.as_ref())
            .await
            .unwrap();
        }

        let rows = interchain_db
            .get_outgoing_message_paths(
                1,
                Some(NaiveDate::from_ymd_opt(2026, 3, 8).unwrap()),
                Some(NaiveDate::from_ymd_opt(2026, 3, 9).unwrap()),
                Some(&[2]),
                true,
            )
            .await
            .unwrap();
        assert_eq!(
            rows,
            vec![MessagePathStatsRow {
                src_chain_id: 1,
                dst_chain_id: 2,
                messages_count: 2
            }]
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn message_paths_omit_zero_mode_keeps_stats_only_behavior() {
        let _db = init_db("message_paths_omit_zero_mode").await;
        let interchain_db = InterchainDatabase::new(_db.client());
        interchain_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: Set(1),
                    name: Set("A".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(2),
                    name: Set("B".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: Set(3),
                    name: Set("C".into()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();
        interchain_db
            .create_or_update_stats_messages(1, 2, 5)
            .await
            .unwrap();

        let rows = interchain_db
            .get_outgoing_message_paths(1, None, None, None, false)
            .await
            .unwrap();
        assert_eq!(
            rows,
            vec![MessagePathStatsRow {
                src_chain_id: 1,
                dst_chain_id: 2,
                messages_count: 5
            }]
        );
    }
}
