use interchain_indexer_entity::{
    bridge_contracts, bridges, chains, crosschain_messages, crosschain_transfers,
    indexer_checkpoints,
};
use parking_lot::RwLock;
use sea_orm::{
    ActiveValue, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, TransactionTrait, prelude::Expr, sea_query::OnConflict,
};
use std::{collections::HashMap, sync::Arc};

use crate::pagination::{MessagePaginationLogic, OutputPagination, PaginationDirection};

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
                        chains::Column::NativeId,
                        chains::Column::Icon,
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

    /// Load a map of native blockchain IDs (normalized with 0x prefix) to chain id.
    ///
    /// This is useful for pre-populating per-batch caches so handlers don't need to
    /// hit the database for every log. Only chains with a non-null `native_id` are
    /// included.
    pub async fn load_native_id_map(&self) -> anyhow::Result<HashMap<String, i64>> {
        chains::Entity::find()
            .filter(chains::Column::NativeId.is_not_null())
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .filter_map(|row| (row.native_id?, row.id).into())
                    .collect::<HashMap<_, _>>()
            })
            .map_err(|e| {
                tracing::error!(err =? e, "Failed to load native id -> chain id map");
                e.into()
            })
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
                                "Bridge with id {} exists but has different name: expected '{}', found '{}'",
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
        page_size: usize,
        last_page: bool,
        input_pagination: Option<MessagePaginationLogic>,
    ) -> anyhow::Result<(
        Vec<(crosschain_messages::Model, Vec<crosschain_transfers::Model>)>,
        OutputPagination<MessagePaginationLogic>,
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

                    let mut pagination = Self::build_pagination_from_messages(
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

    /// Build OutputPagination from a page of messages.
    /// prev_marker and next_marker are built from the first and last element (if exists) respectively.
    /// We must take into account a few query parameters.
    fn build_pagination_from_messages(
        messages: &[crosschain_messages::Model],
        query_direction: PaginationDirection,
        has_more: bool,
        last_page: bool,
    ) -> OutputPagination<MessagePaginationLogic> {
        //We assume that new messages can appear in the database at any time,
        // so the prev marker should always be returned based on the first message
        // (except when there are no messages on the current page).
        let prev_marker = messages.first().map(|msg| MessagePaginationLogic {
            timestamp: msg.init_timestamp,
            message_id: msg.id as u64,
            bridge_id: msg.bridge_id as u32,
            direction: PaginationDirection::Prev,
        });

        // The next marker should not be returned if the last page is requested
        // or if there are no more messages to fetch in the next direction.
        // When the query direction is prev (backward), we assume that
        // the next marker should always be returned.
        let next_marker =
            if !last_page && (query_direction == PaginationDirection::Prev || has_more) {
                messages.last().map(|msg| MessagePaginationLogic {
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

    // VIEW TABLE: crosschain_transfers
    // TBD: add pagination, filters, etc. Current implementation is just for tests only
    pub async fn get_crosschain_transfers(
        &self,
    ) -> anyhow::Result<Vec<crosschain_transfers::Model>> {
        match crosschain_transfers::Entity::find()
            .all(self.db.as_ref())
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => {
                tracing::error!(err =? e, "Failed to fetch crosschain transfers");
                Err(e.into())
            }
        }
    }

    // INDEXER TABLE: indexer_checkpoints
    /// Get checkpoint for a specific bridge and chain
    pub async fn get_checkpoint(
        &self,
        bridge_id: u64,
        chain_id: u64,
    ) -> anyhow::Result<Option<indexer_checkpoints::Model>> {
        indexer_checkpoints::Entity::find()
            .filter(indexer_checkpoints::Column::BridgeId.eq(bridge_id))
            .filter(indexer_checkpoints::Column::ChainId.eq(chain_id))
            .one(self.db.as_ref())
            .await
            .inspect_err(|e| tracing::error!(err =? e, "failed to query checkpoint from database"))
            .map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use interchain_indexer_entity::chains;
    use sea_orm::ActiveValue::Set;

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
            .get_crosschain_messages(None, 100, false, None)
            .await
            .unwrap();
        assert_eq!(crosschain_messages.len(), 4);

        let crosschain_transfers = interchain_db.get_crosschain_transfers().await.unwrap();
        assert_eq!(crosschain_transfers.len(), 5);
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
            native_id: Set(Some(
                "2q9e4r6Mu3U68nU1fYjgbR6JvwrRx36CohpAX5UQxse55x1Q5".to_string(),
            )),
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
        assert_eq!(stored_chain.native_id, ava_chain.native_id.unwrap());
        assert_eq!(stored_chain.icon, ava_chain.icon.unwrap());
    }
}
