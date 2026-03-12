use chrono::{Duration, NaiveDate, NaiveDateTime};
use interchain_indexer_entity::{
    avalanche_icm_blockchain_ids, bridge_contracts, bridges, chains, crosschain_messages,
    crosschain_transfers, indexer_checkpoints, pending_messages,
    sea_orm_active_enums::{EdgeDecimalsSide, MessageStatus, TransferType},
    stats_asset_edges, stats_asset_tokens, stats_assets, stats_chains, tokens,
};
use parking_lot::RwLock;
use sea_orm::{
    ActiveValue, ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait, FromQueryResult,
    JoinType, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
    TransactionTrait,
    entity::prelude::*,
    prelude::Expr,
    sea_query::{Func, OnConflict},
};
use std::{collections::HashMap, sync::Arc};

use crate::pagination::{
    MessagesPaginationLogic, OutputPagination, PaginationDirection, TransfersPaginationLogic,
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

#[derive(Clone)]
pub struct InterchainDatabase {
    pub db: Arc<DatabaseConnection>,

    bridges_names: Arc<RwLock<HashMap<i32, String>>>, // Lazy loaded bridge names
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
        match chains::Entity::find().all(self.db.as_ref()).await {
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
    /// on conflict increments transfers_count and adds to cumulative_amount. Preserves decimals_side.
    pub async fn create_or_update_stats_asset_edge(
        &self,
        stats_asset_id: i64,
        src_chain_id: i64,
        dst_chain_id: i64,
        amount: sea_orm::prelude::BigDecimal,
        decimals_side: EdgeDecimalsSide,
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
                decimals_side: ActiveValue::Set(decimals_side),
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

    /// Updates decimals for an existing edge. Does not change decimals_side.
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
    use chrono::Utc;
    use interchain_indexer_entity::{
        chains, crosschain_messages, crosschain_transfers,
        sea_orm_active_enums::{EdgeDecimalsSide, MessageStatus},
        stats_asset_edges, stats_asset_tokens, stats_assets, stats_chains,
    };
    use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, prelude::BigDecimal};

    use crate::{
        InterchainDatabase,
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
                EdgeDecimalsSide::Source,
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
        assert_eq!(edge.decimals_side, EdgeDecimalsSide::Source);
        assert_eq!(edge.decimals, Some(18));

        interchain_db
            .create_or_update_stats_asset_edge(
                asset.id,
                1,
                2,
                BigDecimal::from(500u64),
                EdgeDecimalsSide::Source,
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
        assert_eq!(edge2.decimals_side, EdgeDecimalsSide::Source);
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
                EdgeDecimalsSide::Destination,
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
        assert_eq!(edge.decimals_side, EdgeDecimalsSide::Destination);

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
        assert_eq!(edge2.decimals_side, EdgeDecimalsSide::Destination);
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
                EdgeDecimalsSide::Source,
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
}
