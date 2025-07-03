use std::sync::Arc;

use crate::models::{self, CrossChainTx};
use anyhow::Ok;
use chrono::Utc;
use sea_orm::{
    ActiveValue, DatabaseConnection, DatabaseTransaction, DbBackend, Statement, TransactionTrait,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use tracing::{instrument, Instrument};
use uuid::Uuid;
use zetachain_cctx_entity::sea_orm_active_enums::ProcessingStatus;
use zetachain_cctx_entity::{
    cctx_status, cross_chain_tx as CrossChainTxEntity, inbound_params,
    outbound_params as OutboundParamsEntity, revert_options,
    sea_orm_active_enums::{
        CctxStatusStatus, CoinType, ConfirmationMode, InboundStatus, Kind, ProtocolContractVersion,
        TxFinalizationStatus,
    },
    watermark,
};
use sea_orm::ConnectionTrait;

pub struct ZetachainCctxDatabase {
    db: Arc<DatabaseConnection>,
}

/// Sanitizes strings by removing null bytes and replacing invalid UTF-8 sequences
/// PostgreSQL UTF-8 encoding doesn't allow null bytes (0x00)
fn sanitize_string(input: String) -> String {
    // Remove null bytes and replace invalid UTF-8 sequences
    input
        .chars()
        .filter(|&c| c != '\0') // Remove null bytes
        .collect::<String>()
        .replace('\u{FFFD}', "?") // Replace replacement character with question mark
}

// Add this struct to hold the complete CCTX data
#[derive(Debug)]
pub struct CompleteCctx {
    pub cctx: CrossChainTxEntity::Model,
    pub status: cctx_status::Model,
    pub inbound: inbound_params::Model,
    pub outbounds: Vec<OutboundParamsEntity::Model>,
    pub revert: revert_options::Model,
}

// Add this struct for the simplified CCTX list view
#[derive(Debug)]
pub struct CctxListItem {
    pub index: String,
    pub status: CctxStatusStatus,
    pub amount: String,
    pub source_chain_id: String,
    pub target_chain_id: String,
}

impl ZetachainCctxDatabase {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn get_cctx_status(&self, cctx_id: i32) -> anyhow::Result<cctx_status::Model> {
        let cctx_status = cctx_status::Entity::find()
            .filter(cctx_status::Column::CrossChainTxId.eq(cctx_id))
            .one(self.db.as_ref())
            .await?;
        cctx_status.ok_or(anyhow::anyhow!("cctx {} has no status", cctx_id))
    }

    pub async fn get_cctx_ids_by_index(
        &self,
        cctxs: Vec<models::CrossChainTx>,
    ) -> anyhow::Result<Vec<(models::CrossChainTx, Option<i32>)>> {
        if cctxs.is_empty() {
            return Ok(vec![]);
        }
        let indices = cctxs
            .iter()
            .map(|cctx| cctx.index.clone())
            .collect::<Vec<String>>();
        let cctx = CrossChainTxEntity::Entity::find()
            .filter(CrossChainTxEntity::Column::Index.is_in(indices.clone()))
            .all(self.db.as_ref())
            .await?;

        // Create a HashMap for quick lookup of existing indices
        let existing_indices: std::collections::HashMap<String, i32> = cctx
            .iter()
            .map(|cctx| (cctx.index.clone(), cctx.id))
            .collect();

        // Map each input index to (index, Option<id>)
        let result = cctxs
            .into_iter()
            .map(|cctx| {
                let id = existing_indices.get(&cctx.index).copied();
                (cctx, id)
            })
            .collect();

        Ok(result)
    }

    pub async fn get_cctx_ids_by_parent_id(
        &self,
        parent_ids: Vec<i32>,
    ) -> anyhow::Result<Vec<i32>> {
        let cctx = CrossChainTxEntity::Entity::find()
            .filter(CrossChainTxEntity::Column::ParentId.is_in(parent_ids))
            .all(self.db.as_ref())
            .await?;
        Ok(cctx.iter().map(|cctx| cctx.id).collect())
    }

    #[instrument(level="debug",skip_all)]
    pub async fn update_cctx_tree_relationships(
        &self,
        cctx_ids: Vec<i32>,
        root_id: i32,
        depth: i32,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        CrossChainTxEntity::Entity::update_many()
            .filter(CrossChainTxEntity::Column::Id.is_in(cctx_ids))
            .set(CrossChainTxEntity::ActiveModel {
                root_id: ActiveValue::Set(Some(root_id)),
                depth: ActiveValue::Set(depth),
                last_status_update_timestamp: ActiveValue::Set(chrono::Utc::now().naive_utc()),
                ..Default::default()
            })
            .exec(tx)
            .await?;
        Ok(())
    }

    #[instrument(level="debug",skip_all)]
    pub async fn traverse_and_update_tree_relationships(
        &self,
        children_cctxs: Vec<CrossChainTx>,
        cctx: CrossChainTxEntity::Model,
    ) -> anyhow::Result<()> {
        let cctx_status = self.get_cctx_status(cctx.id).await?;
        let children_results = self.get_cctx_ids_by_index(children_cctxs).await?;
        let children_ids: Vec<i32> = children_results
            .into_iter()
            .filter_map(|(_, id)| id)
            .collect();
        let tx = self.db.begin().await?;

        let new_root_id = cctx.root_id.unwrap_or(cctx.id);
        if !children_ids.is_empty() {
            let mut frontier = children_ids;
            let mut current_depth = cctx.depth + 1;

            loop {
                let new_frontier = self.get_cctx_ids_by_parent_id(frontier.clone()).await?;
                if new_frontier.is_empty() {
                    break;
                }
                self.update_cctx_tree_relationships(frontier, new_root_id, current_depth, &tx)
                    .await?;
                frontier = new_frontier;
                current_depth += 1;
            }
        }
        // If the cctx is mined, there is no need to do any additional requests
        if cctx_status.status == CctxStatusStatus::OutboundMined {
            self.mark_cctx_tree_processed(cctx.id, &tx).await?;
            tracing::debug!(
                "No children found for CCTX {}, marked as processed",
                cctx.index
            );
        // If cctx is not in the final state, we might get something new in the future, so we will check again later
        } else {
            self.update_last_status_update_timestamp(cctx.id).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn setup_db(&self) -> anyhow::Result<()> {
        tracing::debug!("checking if historical watermark exists");

        //insert historical watermarks if there are no watermarks for historical type
        let historical_watermark = watermark::Entity::find()
            .filter(watermark::Column::Kind.eq(Kind::Historical))
            .one(self.db.as_ref())
            .await?;

        tracing::debug!("removing lock from cctxs");
        CrossChainTxEntity::Entity::update_many()
            .filter(CrossChainTxEntity::Column::ProcessingStatus.eq(ProcessingStatus::Locked))
            .set(CrossChainTxEntity::ActiveModel {
                processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
                ..Default::default()
            })
            .exec(self.db.as_ref())
            .await?;

        if historical_watermark.is_none() {
            tracing::debug!("inserting historical watermark");
            watermark::Entity::insert(watermark::ActiveModel {
                kind: ActiveValue::Set(Kind::Historical),
                processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
                pointer: ActiveValue::Set("MH==".to_string()), //0 in base64
                created_at: ActiveValue::Set(Utc::now().naive_utc()),
                updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                id: ActiveValue::NotSet,
            })
            .exec(self.db.as_ref())
            .await?;
        } else {
            tracing::debug!(
                "historical watermark already exist, pointer: {}",
                historical_watermark.unwrap().pointer
            );
        }
        watermark::Entity::update_many()
            .filter(watermark::Column::ProcessingStatus.eq(ProcessingStatus::Locked))
            .set(watermark::ActiveModel {
                processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
                ..Default::default()
            })
            .exec(self.db.as_ref())
            .await?;

        Ok(())
    }

    #[instrument(level="debug",skip_all)]
    pub async fn move_watermark(
        &self,
        watermark: watermark::Model,
        new_pointer: &str,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        watermark::Entity::update(watermark::ActiveModel {
            id: ActiveValue::Set(watermark.id),
            processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
            pointer: ActiveValue::Set(new_pointer.to_owned()),
            updated_at: ActiveValue::Set(Utc::now().naive_utc()),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark.id))
        .exec(tx)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }
    pub async fn query_cctx(
        &self,
        index: String,
    ) -> anyhow::Result<Option<CrossChainTxEntity::Model>> {
        let cctx = CrossChainTxEntity::Entity::find()
            .filter(CrossChainTxEntity::Column::Index.eq(index))
            .one(self.db.as_ref())
            .await?;

        Ok(cctx)
    }

    #[instrument(level="debug",skip_all)]
    pub async fn create_realtime_watermark(&self, pointer: String) -> anyhow::Result<()> {
        watermark::Entity::insert(watermark::ActiveModel {
            id: ActiveValue::NotSet,
            kind: ActiveValue::Set(Kind::Realtime),
            pointer: ActiveValue::Set(pointer),
            processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
            created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        })
        .exec(self.db.as_ref())
        .instrument(tracing::debug_span!("creating new realtime watermark"))
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

        Ok(())
    }


    #[instrument(,level="info",skip(self,watermark), fields(job_id = %job_id, watermark = %watermark.pointer))]
pub async fn gap_fill(
    self: &ZetachainCctxDatabase,
    job_id: Uuid,
    
    cctxs: Vec<CrossChainTx>,
    next_key: &str,
    watermark: watermark::Model,
    batch_size: u32,
    
) -> anyhow::Result<()> {
    
    let earliest_fetched = cctxs.last().unwrap();
    let latest_synced = self.query_cctx(earliest_fetched.index.clone()).await?;
    if latest_synced.is_some() {
        tracing::debug!("last synced cctx {} is present, skipping", earliest_fetched.index);
    }  else {
        let tx = self.db.begin().await?;
        self.batch_insert_transactions(job_id, &cctxs, &tx).await?;
        self.move_watermark(watermark, next_key, &tx).await?;
        tx.commit().await?;
    }
    Ok(())
}


#[instrument(level="debug",skip_all)]
pub async fn process_realtime_cctxs(
    &self,
    job_id: Uuid,
    cctxs: Vec<CrossChainTx>,
    next_key: &str,
) -> anyhow::Result<()> {
    let tx = self.db.begin().await?;
    //check whether the latest fetched cctx is present in the database
    //we fetch transaction in LIFO order
    let latest_fetched = cctxs.first().unwrap();
    let latest_loaded = self.query_cctx(latest_fetched.index.clone()).await?;


    //if latest fetched cctx is in the db that means that the upper boundary is already covered and there is no new cctxs to save
    if latest_loaded.is_some() {
        tracing::debug!("latest cctx already exists, skipping");
        return Ok(());
    }
    //now we need to check the lower boudary ( the earliest of the fetched cctxs)
    let earliest_fetched = cctxs.last().unwrap();
    let earliest_loaded = self.query_cctx(earliest_fetched.index.clone()).await?;
    
    if earliest_loaded.is_none() {
            // the lower boundary is not covered, so there could be more transaction that happened earlier, that we haven't observed
            //we have to save current pointer to continue fetching until we hit a known transaction
            tracing::debug!("earliest cctx not found, creating new realtime watermark {}", next_key);
            self.create_realtime_watermark(next_key.to_owned()).await?;
    } 

    self.batch_insert_transactions(job_id, &cctxs, &tx).await?;
    
    tx.commit().await?;
    Ok(())
}
#[instrument(level="info",skip_all)]
pub async fn import_cctxs(
    &self,
    job_id: Uuid,
    cctxs: Vec<CrossChainTx>,
    next_key: &str,
    watermark: watermark::Model,
) -> anyhow::Result<()> {
    let tx = self.db.begin().await?;
    self.batch_insert_transactions(job_id, &cctxs, &tx).await?;
    self.move_watermark(watermark, next_key, &tx).await?;
    tx.commit().await?;
    Ok(())
}
    /// Batch insert multiple CrossChainTx records with their child entities
    #[instrument(,level="debug",skip(self, transactions), fields(job_id = %job_id))]
    pub async fn batch_insert_transactions(
        &self,
        job_id: Uuid,
        transactions: &Vec<CrossChainTx>,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        if transactions.is_empty() {
            return Ok(());
        }

        // Prepare batch data for each table
        let mut cctx_models = Vec::new();
        let mut status_models = Vec::new();
        let mut inbound_models = Vec::new();
        let mut outbound_models = Vec::new();
        let mut revert_models = Vec::new();

        // First, prepare all the parent records
        for cctx in transactions {
            let cctx_model = CrossChainTxEntity::ActiveModel {
                id: ActiveValue::NotSet,
                creator: ActiveValue::Set(cctx.creator.clone()),
                index: ActiveValue::Set(cctx.index.clone()),
                processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
                zeta_fees: ActiveValue::Set(cctx.zeta_fees.clone()),
                relayed_message: ActiveValue::Set(Some(cctx.relayed_message.clone())),
                protocol_contract_version: ActiveValue::Set(
                    ProtocolContractVersion::try_from(cctx.protocol_contract_version.clone())
                        .map_err(|e| anyhow::anyhow!(e))?,
                ),
                last_status_update_timestamp: ActiveValue::Set(Utc::now().naive_utc()),
                root_id: ActiveValue::Set(None), //TODO: link to self
                parent_id: ActiveValue::Set(None), //TODO: link to self
                tree_query_flag: ActiveValue::Set(false),
                depth: ActiveValue::Set(0),
            };
            cctx_models.push(cctx_model);
        }

        // Batch insert parent records
        let inserted_cctxs = CrossChainTxEntity::Entity::insert_many(cctx_models)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(CrossChainTxEntity::Column::Index)
                    .do_nothing()
                    .to_owned(),
            )
            .exec_with_returning_many(self.db.as_ref())
            .instrument(tracing::debug_span!("inserting cctxs", job_id = %job_id))
            .await?;

        tracing::debug!("inserted cctxs: {:?}", inserted_cctxs.len());
        // Create a map from index to id for quick lookup
        let index_to_id: std::collections::HashMap<String, i32> = inserted_cctxs
            .into_iter()
            .map(|cctx| (cctx.index, cctx.id))
            .collect();

        // Now prepare child records for each transaction
        for cctx in transactions {
            if let Some(&tx_id) = index_to_id.get(&cctx.index) {
                // Prepare cctx_status
                let status_model = cctx_status::ActiveModel {
                    id: ActiveValue::NotSet,
                    cross_chain_tx_id: ActiveValue::Set(tx_id),
                    status: ActiveValue::Set(
                        CctxStatusStatus::try_from(cctx.cctx_status.status.clone())
                            .map_err(|e| anyhow::anyhow!(e))?,
                    ),
                    status_message: ActiveValue::Set(Some(sanitize_string(
                        cctx.cctx_status.status_message.clone(),
                    ))),
                    error_message: ActiveValue::Set(Some(sanitize_string(
                        cctx.cctx_status.error_message.clone(),
                    ))),
                    last_update_timestamp: ActiveValue::Set(
                        chrono::DateTime::from_timestamp(
                            cctx.cctx_status.last_update_timestamp.parse::<i64>().unwrap(),
                            0,
                        )
                        .unwrap()
                        .naive_utc(),
                    ),
                    is_abort_refunded: ActiveValue::Set(cctx.cctx_status.is_abort_refunded),
                    created_timestamp: ActiveValue::Set(
                        cctx.cctx_status.created_timestamp.parse::<i64>()?,
                    ),
                    error_message_revert: ActiveValue::Set(Some(sanitize_string(
                        cctx.cctx_status.error_message_revert.clone(),
                    ))),
                    error_message_abort: ActiveValue::Set(Some(sanitize_string(
                        cctx.cctx_status.error_message_abort.clone(),
                    ))),
                };
                status_models.push(status_model);

                // Prepare inbound_params
                let inbound_model = inbound_params::ActiveModel {
                    id: ActiveValue::NotSet,
                    cross_chain_tx_id: ActiveValue::Set(tx_id),
                    sender: ActiveValue::Set(cctx.inbound_params.sender.clone()),
                    sender_chain_id: ActiveValue::Set(cctx.inbound_params.sender_chain_id.clone()),
                    tx_origin: ActiveValue::Set(cctx.inbound_params.tx_origin.clone()),
                    coin_type: ActiveValue::Set(
                        CoinType::try_from(cctx.inbound_params.coin_type.clone())
                            .map_err(|e| anyhow::anyhow!(e))?,
                    ),
                    asset: ActiveValue::Set(Some(cctx.inbound_params.asset.clone())),
                    amount: ActiveValue::Set(cctx.inbound_params.amount.clone()),
                    observed_hash: ActiveValue::Set(cctx.inbound_params.observed_hash.clone()),
                    observed_external_height: ActiveValue::Set(
                        cctx.inbound_params.observed_external_height.clone(),
                    ),
                    ballot_index: ActiveValue::Set(cctx.inbound_params.ballot_index.clone()),
                    finalized_zeta_height: ActiveValue::Set(
                        cctx.inbound_params.finalized_zeta_height.clone(),
                    ),
                    tx_finalization_status: ActiveValue::Set(
                        TxFinalizationStatus::try_from(
                            cctx.inbound_params.tx_finalization_status.clone(),
                        )
                        .map_err(|e| anyhow::anyhow!(e))?,
                    ),
                    is_cross_chain_call: ActiveValue::Set(cctx.inbound_params.is_cross_chain_call),
                    status: ActiveValue::Set(
                        InboundStatus::try_from(cctx.inbound_params.status.clone())
                            .map_err(|e| anyhow::anyhow!(e))?,
                    ),
                    confirmation_mode: ActiveValue::Set(
                        ConfirmationMode::try_from(cctx.inbound_params.confirmation_mode.clone())
                            .map_err(|e| anyhow::anyhow!(e))?,
                    ),
                };
                inbound_models.push(inbound_model);

                // Prepare outbound_params
                for outbound in &cctx.outbound_params {
                    let outbound_model = OutboundParamsEntity::ActiveModel {
                        id: ActiveValue::NotSet,
                        cross_chain_tx_id: ActiveValue::Set(tx_id),
                        receiver: ActiveValue::Set(outbound.receiver.clone()),
                        receiver_chain_id: ActiveValue::Set(outbound.receiver_chain_id.clone()),
                        coin_type: ActiveValue::Set(
                            CoinType::try_from(outbound.coin_type.clone())
                                .map_err(|e| anyhow::anyhow!(e))?,
                        ),
                        amount: ActiveValue::Set(outbound.amount.clone()),
                        tss_nonce: ActiveValue::Set(outbound.tss_nonce.clone()),
                        gas_limit: ActiveValue::Set(outbound.gas_limit.clone()),
                        gas_price: ActiveValue::Set(Some(outbound.gas_price.clone())),
                        gas_priority_fee: ActiveValue::Set(Some(outbound.gas_priority_fee.clone())),
                        hash: ActiveValue::Set(outbound.hash.clone()),
                        ballot_index: ActiveValue::Set(Some(outbound.ballot_index.clone())),
                        observed_external_height: ActiveValue::Set(
                            outbound.observed_external_height.clone(),
                        ),
                        gas_used: ActiveValue::Set(outbound.gas_used.clone()),
                        effective_gas_price: ActiveValue::Set(outbound.effective_gas_price.clone()),
                        effective_gas_limit: ActiveValue::Set(outbound.effective_gas_limit.clone()),
                        tss_pubkey: ActiveValue::Set(outbound.tss_pubkey.clone()),
                        tx_finalization_status: ActiveValue::Set(
                            TxFinalizationStatus::try_from(outbound.tx_finalization_status.clone())
                                .map_err(|e| anyhow::anyhow!(e))?,
                        ),
                        call_options_gas_limit: ActiveValue::Set(
                            outbound.call_options.clone().map(|c| c.gas_limit.clone()),
                        ),
                        call_options_is_arbitrary_call: ActiveValue::Set(
                            outbound.call_options.clone().map(|c| c.is_arbitrary_call),
                        ),
                        confirmation_mode: ActiveValue::Set(
                            ConfirmationMode::try_from(outbound.confirmation_mode.clone())
                                .map_err(|e| anyhow::anyhow!(e))?,
                        ),
                    };
                    outbound_models.push(outbound_model);
                }

                // Prepare revert_options
                let revert_model = revert_options::ActiveModel {
                    id: ActiveValue::NotSet,
                    cross_chain_tx_id: ActiveValue::Set(tx_id),
                    revert_address: ActiveValue::Set(Some(
                        cctx.revert_options.revert_address.clone(),
                    )),
                    call_on_revert: ActiveValue::Set(cctx.revert_options.call_on_revert),
                    abort_address: ActiveValue::Set(Some(cctx.revert_options.abort_address.clone())),
                    revert_message: ActiveValue::Set(cctx.revert_options.revert_message.clone()),
                    revert_gas_limit: ActiveValue::Set(cctx.revert_options.revert_gas_limit.clone()),
                };
                revert_models.push(revert_model);
            }
        }

        // Batch insert all child records
        if !status_models.is_empty() {
            cctx_status::Entity::insert_many(status_models)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::column(cctx_status::Column::CrossChainTxId)
                        .do_nothing()
                        .to_owned(),
                )
                .exec(tx)
                .instrument(tracing::debug_span!("inserting cctx_status"))
                .await?;
        }

        Ok(())
    }

    #[instrument(,level="debug",skip(self), fields(watermark_id = %watermark.id))]
    pub async fn unlock_watermark(&self, watermark: watermark::Model) -> anyhow::Result<()> {
        let res = watermark::Entity::update(watermark::ActiveModel {
            id: ActiveValue::Unchanged(watermark.id),
            processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
            updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark.id))
        .exec(self.db.as_ref())
        .instrument(tracing::debug_span!("unlocking watermark", watermark_id = %watermark.id))
        .await?;

        tracing::debug!("unlocked watermark: {:?}", res);
        Ok(())
    }

    pub async fn lock_watermark(&self, watermark: watermark::Model) -> anyhow::Result<()> {
        watermark::Entity::update(watermark::ActiveModel {
            id: ActiveValue::Unchanged(watermark.id),
            processing_status: ActiveValue::Set(ProcessingStatus::Locked),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark.id))
        .exec(self.db.as_ref())
        .await?;
        Ok(())
    }

    pub async fn delete_watermark(&self, watermark: watermark::Model) -> anyhow::Result<()> {
        watermark::Entity::delete(watermark::ActiveModel {
            id: ActiveValue::Unchanged(watermark.id),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark.id))
        .exec(self.db.as_ref())
        .await?;
        Ok(())
    }

    pub async fn unlock_cctx(&self, id: i32) -> anyhow::Result<()> {
        CrossChainTxEntity::Entity::update(CrossChainTxEntity::ActiveModel {
            id: ActiveValue::Unchanged(id),
            processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
            ..Default::default()
        })
        .filter(CrossChainTxEntity::Column::Id.eq(id))
        .exec(self.db.as_ref())
        .await?;
        Ok(())
    }
    #[instrument(,level="debug",skip(self), fields(job_id = %job_id))]
    pub async fn query_cctxs_for_status_update(
        &self,
        batch_size: u32,
        job_id: Uuid,
    ) -> anyhow::Result<Vec<(i32, String)>> {
        let statement = format!(
            r#"
        WITH cctxs AS (
            SELECT cctx.id, cctx.index, cctx.last_status_update_timestamp
            FROM cross_chain_tx cctx
            JOIN cctx_status cs ON cctx.id = cs.cross_chain_tx_id
            WHERE (cs.status IN ('PendingInbound', 'PendingOutbound', 'PendingRevert') OR cctx.tree_query_flag = false)
            AND cctx.processing_status = 'unlocked'::processing_status
            ORDER BY cctx.last_status_update_timestamp ASC
            LIMIT {batch_size}
            FOR UPDATE SKIP LOCKED
        )
        UPDATE cross_chain_tx cctx
        SET processing_status = 'locked'::processing_status, last_status_update_timestamp = NOW()
        WHERE id IN (SELECT id FROM cctxs)
        RETURNING id, index
        "#
        );

        let statement = Statement::from_sql_and_values(DbBackend::Postgres, statement, vec![]);

        let rows: Vec<(i32, String)> = self.db.query_all(statement).await?.into_iter().map(|r| (r.try_get_by_index(0).unwrap(), r.try_get_by_index(1).unwrap())).collect();
        Ok(rows)
    }

    pub async fn mark_cctx_tree_processed(
        &self,
        cctx_id: i32,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        CrossChainTxEntity::Entity::update(CrossChainTxEntity::ActiveModel {
            id: ActiveValue::Set(cctx_id),
            tree_query_flag: ActiveValue::Set(true),
            last_status_update_timestamp: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        })
        .filter(CrossChainTxEntity::Column::Id.eq(cctx_id))
        .exec(tx)
        .await?;
        Ok(())
    }

    pub async fn update_last_status_update_timestamp(&self, cctx_id: i32) -> anyhow::Result<()> {
        CrossChainTxEntity::Entity::update(CrossChainTxEntity::ActiveModel {
            id: ActiveValue::Set(cctx_id),
            last_status_update_timestamp: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
            ..Default::default()
        })
        .filter(CrossChainTxEntity::Column::Id.eq(cctx_id))
        .exec(self.db.as_ref())
        .await?;
        Ok(())
    }

    #[instrument(,level="info",skip_all, fields(job_id = %job_id))]
    pub async fn update_cctx_status(
        &self,
        job_id: Uuid,
        cctx_id: i32,
        fetched_cctx: CrossChainTx,
    ) -> anyhow::Result<()> {
        let outbound_params = fetched_cctx.outbound_params;

        let tx = self.db.begin().await?;
        for outbound_params in outbound_params {
            if let Some(record) = OutboundParamsEntity::Entity::find()
                .filter(OutboundParamsEntity::Column::CrossChainTxId.eq(Some(cctx_id)))
                .filter(OutboundParamsEntity::Column::Receiver.eq(outbound_params.receiver))
                .one(self.db.as_ref())
                .await
                .unwrap()
            {
                OutboundParamsEntity::Entity::update(OutboundParamsEntity::ActiveModel {
                    id: ActiveValue::Set(record.id),
                    tx_finalization_status: ActiveValue::Set(
                        TxFinalizationStatus::try_from(outbound_params.tx_finalization_status)
                            .map_err(|e| anyhow::anyhow!(e))?,
                    ),
                    gas_used: ActiveValue::Set(outbound_params.gas_used),
                    effective_gas_price: ActiveValue::Set(outbound_params.effective_gas_price),
                    effective_gas_limit: ActiveValue::Set(outbound_params.effective_gas_limit),
                    tss_nonce: ActiveValue::Set(outbound_params.tss_nonce),
                    ..Default::default()
                })
                .filter(OutboundParamsEntity::Column::Id.eq(record.id))
                .exec(self.db.as_ref())
                .await?;
            }
        }

        if let Some(cctx_status_row) = cctx_status::Entity::find()
            .filter(cctx_status::Column::CrossChainTxId.eq(cctx_id))
            .one(self.db.as_ref())
            .await? {
                cctx_status::Entity::update(cctx_status::ActiveModel {
                    id: ActiveValue::Set(cctx_status_row.id),
                    status: ActiveValue::Set(CctxStatusStatus::try_from(fetched_cctx.cctx_status.status.clone())
                        .map_err(|e| anyhow::anyhow!(e))?
                    ),
                    last_update_timestamp: ActiveValue::Set(chrono::Utc::now().naive_utc()),
                    ..Default::default()
                })
                .filter(cctx_status::Column::Id.eq(cctx_status_row.id))
                .exec(self.db.as_ref())
                .await?;
            }

        
        //unlock cctx
        CrossChainTxEntity::Entity::update(CrossChainTxEntity::ActiveModel {
            id: ActiveValue::Set(cctx_id),
            processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
            ..Default::default()
        })
        .filter(CrossChainTxEntity::Column::Id.eq(cctx_id))
        .exec(&tx)
        .await?;

        //commit transaction
        tx.commit().await?;

        Ok(())
    }
    #[instrument(,level="debug",skip(self, tx), fields(job_id = %job_id))]
    pub async fn insert_transaction_with_tree_relationships(
        &self,
        tx: CrossChainTx,
        job_id: Uuid,
        root_id: i32,
        parent_id: i32,
        depth: i32,
    ) -> anyhow::Result<()> {
        // Insert main cross_chain_tx record
        let index = tx.index.clone();
        let cctx_model = CrossChainTxEntity::ActiveModel {
            id: ActiveValue::NotSet,
            creator: ActiveValue::Set(tx.creator),
            index: ActiveValue::Set(tx.index),
            processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
            zeta_fees: ActiveValue::Set(tx.zeta_fees),
            relayed_message: ActiveValue::Set(Some(tx.relayed_message)),
            protocol_contract_version: ActiveValue::Set(
                ProtocolContractVersion::try_from(tx.protocol_contract_version)
                    .map_err(|e| anyhow::anyhow!(e))?,
            ),
            last_status_update_timestamp: ActiveValue::Set(Utc::now().naive_utc()),
            root_id: ActiveValue::Set(Some(root_id)),
            parent_id: ActiveValue::Set(Some(parent_id)),
            tree_query_flag: ActiveValue::Set(false),
            depth: ActiveValue::Set(depth),
        };

        let tx_result = CrossChainTxEntity::Entity::insert(cctx_model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(CrossChainTxEntity::Column::Index)
                    .do_nothing()
                    .to_owned(),
            )
            .exec(self.db.as_ref())
            .instrument(tracing::debug_span!("inserting cctx", index = %index, job_id = %job_id))
            .await?;

        // Get the inserted tx id
        let tx_id = tx_result.last_insert_id;

        // Insert cctx_status
        let status_model = cctx_status::ActiveModel {
            id: ActiveValue::NotSet,
            cross_chain_tx_id: ActiveValue::Set(tx_id),
            status: ActiveValue::Set(
                CctxStatusStatus::try_from(tx.cctx_status.status.clone())
                    .map_err(|e| anyhow::anyhow!(e))?,
            ),
            status_message: ActiveValue::Set(Some(sanitize_string(tx.cctx_status.status_message))),
            error_message: ActiveValue::Set(Some(sanitize_string(tx.cctx_status.error_message))),
            last_update_timestamp: ActiveValue::Set(
                // parse NaiveDateTime from epoch seconds
                chrono::DateTime::from_timestamp(
                    tx.cctx_status.last_update_timestamp.parse::<i64>().unwrap(),
                    0,
                )
                .unwrap()
                .naive_utc(),
            ),
            is_abort_refunded: ActiveValue::Set(tx.cctx_status.is_abort_refunded),
            created_timestamp: ActiveValue::Set(tx.cctx_status.created_timestamp.parse::<i64>()?),
            error_message_revert: ActiveValue::Set(Some(sanitize_string(
                tx.cctx_status.error_message_revert,
            ))),
            error_message_abort: ActiveValue::Set(Some(sanitize_string(
                tx.cctx_status.error_message_abort,
            ))),
        };
        cctx_status::Entity::insert(status_model)
            .exec(self.db.as_ref())
            .instrument(
                tracing::debug_span!("inserting cctx_status", index = %index, job_id = %job_id),
            )
            .await?;

        // Insert inbound_params
        let inbound_model = inbound_params::ActiveModel {
            id: ActiveValue::NotSet,
            cross_chain_tx_id: ActiveValue::Set(tx_id),
            sender: ActiveValue::Set(tx.inbound_params.sender),
            sender_chain_id: ActiveValue::Set(tx.inbound_params.sender_chain_id),
            tx_origin: ActiveValue::Set(tx.inbound_params.tx_origin),
            coin_type: ActiveValue::Set(
                CoinType::try_from(tx.inbound_params.coin_type).map_err(|e| anyhow::anyhow!(e))?,
            ),
            asset: ActiveValue::Set(Some(tx.inbound_params.asset)),
            amount: ActiveValue::Set(tx.inbound_params.amount),
            observed_hash: ActiveValue::Set(tx.inbound_params.observed_hash),
            observed_external_height: ActiveValue::Set(tx.inbound_params.observed_external_height),
            ballot_index: ActiveValue::Set(tx.inbound_params.ballot_index),
            finalized_zeta_height: ActiveValue::Set(tx.inbound_params.finalized_zeta_height),
            tx_finalization_status: ActiveValue::Set(
                TxFinalizationStatus::try_from(tx.inbound_params.tx_finalization_status)
                    .map_err(|e| anyhow::anyhow!(e))?,
            ),
            is_cross_chain_call: ActiveValue::Set(tx.inbound_params.is_cross_chain_call),
            status: ActiveValue::Set(
                InboundStatus::try_from(tx.inbound_params.status)
                    .map_err(|e| anyhow::anyhow!(e))?,
            ),
            confirmation_mode: ActiveValue::Set(
                ConfirmationMode::try_from(tx.inbound_params.confirmation_mode)
                    .map_err(|e| anyhow::anyhow!(e))?,
            ),
        };
        inbound_params::Entity::insert(inbound_model)
            .exec(self.db.as_ref())
            .instrument(
                tracing::debug_span!("inserting inbound_params", index = %index, job_id = %job_id),
            )
            .await?;

        // Insert outbound_params
        for outbound in tx.outbound_params {
            let outbound_model = OutboundParamsEntity::ActiveModel {
                id: ActiveValue::NotSet,
                cross_chain_tx_id: ActiveValue::Set(tx_id),
                receiver: ActiveValue::Set(outbound.receiver),
                receiver_chain_id: ActiveValue::Set(outbound.receiver_chain_id),
                coin_type: ActiveValue::Set(
                    CoinType::try_from(outbound.coin_type).map_err(|e| anyhow::anyhow!(e))?,
                ),
                amount: ActiveValue::Set(outbound.amount),
                tss_nonce: ActiveValue::Set(outbound.tss_nonce),
                gas_limit: ActiveValue::Set(outbound.gas_limit),
                gas_price: ActiveValue::Set(Some(outbound.gas_price)),
                gas_priority_fee: ActiveValue::Set(Some(outbound.gas_priority_fee)),
                hash: ActiveValue::Set(outbound.hash),
                ballot_index: ActiveValue::Set(Some(outbound.ballot_index)),
                observed_external_height: ActiveValue::Set(outbound.observed_external_height),
                gas_used: ActiveValue::Set(outbound.gas_used),
                effective_gas_price: ActiveValue::Set(outbound.effective_gas_price),
                effective_gas_limit: ActiveValue::Set(outbound.effective_gas_limit),
                tss_pubkey: ActiveValue::Set(outbound.tss_pubkey),
                tx_finalization_status: ActiveValue::Set(
                    TxFinalizationStatus::try_from(outbound.tx_finalization_status)
                        .map_err(|e| anyhow::anyhow!(e))?,
                ),
                call_options_gas_limit: ActiveValue::Set(
                    outbound.call_options.clone().map(|c| c.gas_limit),
                ),
                call_options_is_arbitrary_call: ActiveValue::Set(
                    outbound.call_options.map(|c| c.is_arbitrary_call),
                ),
                confirmation_mode: ActiveValue::Set(
                    ConfirmationMode::try_from(outbound.confirmation_mode)
                        .map_err(|e| anyhow::anyhow!(e))?,
                ),
            };
            OutboundParamsEntity::Entity::insert(outbound_model)
            .exec(self.db.as_ref())
            .instrument(tracing::debug_span!("inserting outbound_params", index = %index, job_id = %job_id))
            .await?;
        }

        // Insert revert_options
        let revert_model = revert_options::ActiveModel {
            id: ActiveValue::NotSet,
            cross_chain_tx_id: ActiveValue::Set(tx_id),
            revert_address: ActiveValue::Set(Some(tx.revert_options.revert_address)),
            call_on_revert: ActiveValue::Set(tx.revert_options.call_on_revert),
            abort_address: ActiveValue::Set(Some(tx.revert_options.abort_address)),
            revert_message: ActiveValue::Set(tx.revert_options.revert_message),
            revert_gas_limit: ActiveValue::Set(tx.revert_options.revert_gas_limit),
        };
        revert_options::Entity::insert(revert_model)
            .exec(self.db.as_ref())
            .instrument(
                tracing::debug_span!("inserting revert_options", index = %index, job_id = %job_id),
            )
            .await?;

        Ok(())
    }

    pub async fn get_complete_cctx(&self, index: String) -> anyhow::Result<Option<CompleteCctx>> {
        // First get the main CCTX
        let cctx = CrossChainTxEntity::Entity::find()
            .filter(CrossChainTxEntity::Column::Index.eq(&index))
            .one(self.db.as_ref())
            .await?;

        let cctx = match cctx {
            Some(cctx) => cctx,
            None => return Ok(None),
        };

        // Get all related entities
        let status = cctx_status::Entity::find()
            .filter(cctx_status::Column::CrossChainTxId.eq(cctx.id))
            .one(self.db.as_ref())
            .await?;

        let inbound = inbound_params::Entity::find()
            .filter(inbound_params::Column::CrossChainTxId.eq(cctx.id))
            .one(self.db.as_ref())
            .await?;

        let outbounds = OutboundParamsEntity::Entity::find()
            .filter(OutboundParamsEntity::Column::CrossChainTxId.eq(cctx.id))
            .all(self.db.as_ref())
            .await?;

        let revert = revert_options::Entity::find()
            .filter(revert_options::Column::CrossChainTxId.eq(cctx.id))
            .one(self.db.as_ref())
            .await?;

        // Ensure all required entities exist
        let status =
            status.ok_or_else(|| anyhow::anyhow!("CCTX status not found for index: {}", index))?;
        let inbound = inbound
            .ok_or_else(|| anyhow::anyhow!("Inbound params not found for index: {}", index))?;
        let revert = revert
            .ok_or_else(|| anyhow::anyhow!("Revert options not found for index: {}", index))?;

        Ok(Some(CompleteCctx {
            cctx,
            status,
            inbound,
            outbounds,
            revert,
        }))
    }

    pub async fn list_cctxs(
        &self,
        limit: i64,
        status_filter: Option<String>,
    ) -> anyhow::Result<Vec<CctxListItem>> {
        // First, get CCTX records with their status
        let mut query = CrossChainTxEntity::Entity::find()
            .inner_join(cctx_status::Entity)
            .limit(limit as u64);

        // Apply status filter if provided
        if let Some(status_str) = status_filter {
            let status_enum = CctxStatusStatus::try_from(status_str.clone())
                .map_err(|_| anyhow::anyhow!("Invalid status: {}", status_str))?;
            query = query.filter(cctx_status::Column::Status.eq(status_enum));
        }

        // Get the CCTX records
        let cctxs = query.all(self.db.as_ref()).await?;

        // For each CCTX, get the status, inbound and outbound params
        let mut items = Vec::new();
        for cctx in cctxs {
            // Get status
            let status = cctx_status::Entity::find()
                .filter(cctx_status::Column::CrossChainTxId.eq(cctx.id))
                .one(self.db.as_ref())
                .await?;

            // Get inbound params
            let inbound = inbound_params::Entity::find()
                .filter(inbound_params::Column::CrossChainTxId.eq(cctx.id))
                .one(self.db.as_ref())
                .await?;

            // Get outbound params
            let outbounds = OutboundParamsEntity::Entity::find()
                .filter(OutboundParamsEntity::Column::CrossChainTxId.eq(cctx.id))
                .all(self.db.as_ref())
                .await?;

            if let (Some(status), Some(inbound)) = (status, inbound) {
                // Use the first outbound for amount and target chain ID
                if let Some(outbound) = outbounds.first() {
                    items.push(CctxListItem {
                        index: cctx.index,
                        status: status.status,
                        amount: outbound.amount.clone(),
                        source_chain_id: inbound.sender_chain_id.clone(),
                        target_chain_id: outbound.receiver_chain_id.clone(),
                    });
                }
            }
        }

        Ok(items)
    }
}
