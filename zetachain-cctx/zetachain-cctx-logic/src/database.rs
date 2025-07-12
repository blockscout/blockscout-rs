use std::collections::HashSet;
use std::sync::Arc;

use crate::models::{
    self, CctxListItem, CctxShort, CompleteCctx, CrossChainTx, RelatedCctx, RelatedOutboundParams,
};
use anyhow::Ok;
use chrono::{NaiveDateTime, Utc};
use sea_orm::ConnectionTrait;
use sea_orm::{
    ActiveValue, DatabaseConnection, DatabaseTransaction, DbBackend, Statement, TransactionTrait,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use tracing::{instrument, Instrument};
use uuid::Uuid;
use zetachain_cctx_entity::sea_orm_active_enums::ProcessingStatus;
use zetachain_cctx_entity::{
    cctx_status, cross_chain_tx as CrossChainTxEntity, inbound_params as InboundParamsEntity,
    outbound_params as OutboundParamsEntity, revert_options as RevertOptionsEntity,
    sea_orm_active_enums::{
        CctxStatusStatus, CoinType, ConfirmationMode, InboundStatus, Kind, ProtocolContractVersion,
        TxFinalizationStatus,
    },
    watermark,
};

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

impl ZetachainCctxDatabase {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn mark_watermark_as_failed(
        &self,
        watermark: watermark::Model,
    ) -> anyhow::Result<()> {
        watermark::Entity::update(watermark::ActiveModel {
            id: ActiveValue::Set(watermark.id),
            processing_status: ActiveValue::Set(ProcessingStatus::Failed),
            retries_number: ActiveValue::Set(watermark.retries_number + 1),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark.id))
        .exec(self.db.as_ref())
        .await?;
        Ok(())
    }

    pub async fn get_cctx_status(&self, cctx_id: i32) -> anyhow::Result<cctx_status::Model> {
        let cctx_status = cctx_status::Entity::find()
            .filter(cctx_status::Column::CrossChainTxId.eq(cctx_id))
            .one(self.db.as_ref())
            .await?;
        cctx_status.ok_or(anyhow::anyhow!("cctx {} has no status", cctx_id))
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn mark_cctx_as_failed(&self, cctx: &CctxShort) -> anyhow::Result<()> {
        CrossChainTxEntity::Entity::update(CrossChainTxEntity::ActiveModel {
            id: ActiveValue::Set(cctx.id),
            processing_status: ActiveValue::Set(ProcessingStatus::Failed),
            retries_number: ActiveValue::Set(cctx.retries_number),
            ..Default::default()
        })
        .filter(CrossChainTxEntity::Column::Id.eq(cctx.id))
        .exec(self.db.as_ref())
        .await?;
        Ok(())
    }

    pub async fn find_outdated_children_by_index(
        &self,
        fetched_cctxs: Vec<models::CrossChainTx>,
        parent_id: i32,
        root_id: i32,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<(Vec<models::CrossChainTx>, Vec<i32>)> {
        if fetched_cctxs.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let mut need_to_import = Vec::new();
        let mut need_to_update_ids = Vec::new();

        for fetched_cctx in fetched_cctxs {
            let cctx = CrossChainTxEntity::Entity::find()
                .filter(CrossChainTxEntity::Column::Index.eq(fetched_cctx.index.clone()))
                .one(tx)
                .await?;
            if cctx.is_some() {
                let cctx = cctx.unwrap();
                if cctx.parent_id != Some(parent_id) || cctx.root_id != Some(root_id) {
                    need_to_update_ids.push(cctx.id);
                }
            } else {
                need_to_import.push(fetched_cctx);
            }
        }

        Ok((need_to_import, need_to_update_ids))
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn get_cctx_ids_by_parent_id(
        &self,
        parent_ids: Vec<i32>,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<Vec<i32>> {
        let cctx = CrossChainTxEntity::Entity::find()
            .filter(CrossChainTxEntity::Column::ParentId.is_in(parent_ids))
            .all(tx)
            .await?;

        Ok(cctx.iter().map(|cctx| cctx.id).collect())
    }

    pub async fn update_multiple_related_cctxs(
        &self,
        cctx_ids: Vec<i32>,
        root_id: i32,
        depth: i32,
        parent_id: i32,
        job_id: Uuid,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        tracing::debug!(
            "job_id: {} updating multiple related cctxs: {:?} setting parent_id to {:?}",
            job_id,
            cctx_ids,
            parent_id
        );
        CrossChainTxEntity::Entity::update_many()
            .filter(CrossChainTxEntity::Column::Id.is_in(cctx_ids.iter().cloned()))
            .set(CrossChainTxEntity::ActiveModel {
                root_id: ActiveValue::Set(Some(root_id)),
                depth: ActiveValue::Set(depth),
                parent_id: ActiveValue::Set(Some(parent_id)),
                last_status_update_timestamp: ActiveValue::Set(chrono::Utc::now().naive_utc()),
                updated_by: ActiveValue::Set(job_id.to_string()),
                ..Default::default()
            })
            .exec(tx)
            .await?;
        Ok(())
    }
    #[instrument(level = "debug", skip_all,fields(child_index = %child_index, child_id = %child_id, root_id = %root_id, parent_id = %parent_id, depth = %depth))]
    pub async fn update_single_related_cctx(
        &self,
        child_index: &str,
        child_id: i32,
        root_id: i32,
        depth: i32,
        parent_id: i32,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        CrossChainTxEntity::Entity::update_many()
            .filter(CrossChainTxEntity::Column::Id.eq(child_id))
            .set(CrossChainTxEntity::ActiveModel {
                root_id: ActiveValue::Set(Some(root_id)),
                depth: ActiveValue::Set(depth),
                parent_id: ActiveValue::Set(Some(parent_id)),
                last_status_update_timestamp: ActiveValue::Set(chrono::Utc::now().naive_utc()),
                ..Default::default()
            })
            .exec(tx)
            .await?;
        Ok(())
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn traverse_and_update_tree_relationships(
        &self,
        children_cctxs: Vec<CrossChainTx>,
        cctx: &CctxShort,
        job_id: Uuid,
    ) -> anyhow::Result<()> {
        let cctx_status = self.get_cctx_status(cctx.id).await?;
        let root_id = cctx.root_id.unwrap_or(cctx.id);
        let tx = self.db.begin().await?;
        let (need_to_import, need_to_update_ids) = self
            .find_outdated_children_by_index(children_cctxs, cctx.id, root_id, &tx)
            .await?;
        tracing::debug!(
            "need_to_import: {:?}, need_to_update_ids: {:?}",
            need_to_import,
            need_to_update_ids
        );

        for new_cctx in need_to_import {
            self.import_related_cctx(new_cctx, job_id, root_id, cctx.id, cctx.depth + 1, &tx)
                .await?;
        }
        //First we update parent_id for the direct descendants
        //The root_id for both direct and indirect descendants will be inside the loop
        CrossChainTxEntity::Entity::update_many()
            .filter(CrossChainTxEntity::Column::Id.is_in(need_to_update_ids.iter().cloned()))
            .set(CrossChainTxEntity::ActiveModel {
                parent_id: ActiveValue::Set(Some(cctx.id)),
                depth: ActiveValue::Set(cctx.depth + 1),
                root_id: ActiveValue::Set(Some(root_id)),
                ..Default::default()
            })
            .exec(&tx)
            .await?;

        let mut frontier = need_to_update_ids.clone();
        let mut current_depth = cctx.depth + 1;

        loop {
            let mut new_frontier: Vec<i32> = vec![];
            for id in frontier.iter() {
                let children = self.get_cctx_ids_by_parent_id(vec![*id], &tx).await?;
                new_frontier.extend(children.into_iter());
            }
            if new_frontier.is_empty() {
                break;
            }
            println!("new_frontier: {:?}", new_frontier);
            frontier = new_frontier;

            //Now we update the root_id for all of the descendants
            CrossChainTxEntity::Entity::update_many()
                .filter(CrossChainTxEntity::Column::Id.is_in(frontier.iter().cloned()))
                .set(CrossChainTxEntity::ActiveModel {
                    root_id: ActiveValue::Set(Some(root_id)),
                    depth: ActiveValue::Set(current_depth),
                    ..Default::default()
                })
                .exec(&tx)
                .await?;
            current_depth += 1;
        }

        // If the cctx is mined, there is no need to do any additional requests
        if cctx_status.status == CctxStatusStatus::OutboundMined {
            tracing::debug!("cctx {} is mined, marking as processed", cctx.index);
            self.mark_cctx_tree_processed(cctx.id, job_id, &cctx.index, &tx)
                .await?;
            tracing::debug!(
                "No children found for CCTX {}, marked as processed",
                cctx.index
            );
        // If cctx is not in the final state, we might get something new in the future, so we will check again later
        } else {
            tracing::debug!(
                "cctx {} is not mined, updating last status update timestamp",
                cctx.index
            );
            self.update_last_status_update_timestamp(cctx.id, job_id, &tx)
                .await?;
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
                updated_by: ActiveValue::Set("test".to_string()),
                ..Default::default()
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

    #[instrument(skip(self,tx,watermark),level="debug",fields(watermark_id = %watermark.id, new_pointer = %new_pointer))]
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

    #[instrument(level = "debug", skip_all)]
    pub async fn create_realtime_watermark(
        &self,
        pointer: String,
        job_id: Uuid,
    ) -> anyhow::Result<()> {
        watermark::Entity::insert(watermark::ActiveModel {
            id: ActiveValue::NotSet,
            kind: ActiveValue::Set(Kind::Realtime),
            pointer: ActiveValue::Set(pointer),
            processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
            created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            updated_by: ActiveValue::Set(job_id.to_string()),
            ..Default::default()
        })
        .exec(self.db.as_ref())
        .instrument(tracing::debug_span!("creating new realtime watermark"))
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

        Ok(())
    }

    #[instrument(,level="debug",skip_all, fields(cctx_indexes = %cctxs.iter().map(|cctx| cctx.index.clone()).collect::<Vec<String>>().join(",")))]
    pub async fn import_missing_cctxs(
        self: &ZetachainCctxDatabase,
        job_id: Uuid,
        cctxs: Vec<CrossChainTx>,
        next_key: &str,
        realtime_threshold: i64,
        watermark: watermark::Model,
    ) -> anyhow::Result<(ProcessingStatus, String)> {
        let earliest_fetched = cctxs.last().unwrap();
        let latest_fetched = cctxs.first().unwrap();
        if earliest_fetched
            .cctx_status
            .last_update_timestamp
            .parse::<i64>()
            .unwrap_or(0)
            < Utc::now().timestamp() - realtime_threshold
        {
            tracing::info!(
                "earliest fetched cctx is too old, skipping and marking watermark as done"
            );
            return Ok((ProcessingStatus::Done, watermark.pointer.clone()));
        }
        let latest_synced = self.query_cctx(latest_fetched.index.clone()).await?;
        if latest_synced.is_some() {
            tracing::debug!(
                "last synced cctx {} is present, marking as done",
                earliest_fetched.index
            );
            return Ok((ProcessingStatus::Done, watermark.pointer));
        } else {
            tracing::debug!("last synced cctx is absent, inserting a batch and moving watermark");
            let tx = self.db.begin().await?;
            self.batch_insert_transactions(job_id, &cctxs, &tx).await?;
            self.move_watermark(watermark, next_key, &tx).await?;
            tx.commit().await?;
            return Ok((ProcessingStatus::Unlocked, next_key.to_owned()));
        }
    }

    pub async fn update_watermark(
        &self,
        watermark: watermark::Model,
        pointer: &str,
        status: ProcessingStatus,
    ) -> anyhow::Result<()> {
        watermark::Entity::update(watermark::ActiveModel {
            id: ActiveValue::Unchanged(watermark.id),
            processing_status: ActiveValue::Set(status),
            pointer: ActiveValue::Set(pointer.to_owned()),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark.id))
        .exec(self.db.as_ref())
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }
    #[instrument(level = "debug", skip_all)]
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
            tracing::debug!(
                " latest cctx {} for watermark {} already exists, skipping",
                latest_fetched.index,
                next_key
            );
            return Ok(());
        }
        //now we need to check the lower boudary ( the earliest of the fetched cctxs)
        let earliest_fetched = cctxs.last().unwrap();
        let earliest_loaded = self.query_cctx(earliest_fetched.index.clone()).await?;

        if earliest_loaded.is_none() {
            // the lower boundary is not covered, so there could be more transaction that happened earlier, that we haven't observed
            //we have to save current pointer to continue fetching until we hit a known transaction
            tracing::debug!(
                "earliest cctx not found, creating new realtime watermark {}",
                next_key
            );
            self.create_realtime_watermark(next_key.to_owned(), job_id)
                .await?;
        }

        self.batch_insert_transactions(job_id, &cctxs, &tx).await?;

        tx.commit().await?;
        Ok(())
    }
    #[instrument(level = "debug", skip_all)]
    pub async fn import_cctxs(
        &self,
        job_id: Uuid,
        cctxs: Vec<CrossChainTx>,
        next_key: &str,
        watermark: watermark::Model,
    ) -> anyhow::Result<()> {
        let tx = self.db.begin().await?;
        self.batch_insert_transactions(job_id, &cctxs, &tx).await?;
        self.update_watermark(watermark, next_key, ProcessingStatus::Unlocked)
            .await?;
        tx.commit().await?;
        Ok(())
    }
    /// Batch insert multiple CrossChainTx records with their child entities
    #[instrument(skip_all, level = "debug")]
    pub async fn batch_insert_transactions(
        &self,
        job_id: Uuid,
        cctxs: &Vec<CrossChainTx>,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        if cctxs.is_empty() {
            return Ok(());
        }

        // Prepare batch data for each table
        let mut cctx_models = Vec::new();
        let mut status_models = Vec::new();
        let mut inbound_models = Vec::new();
        let mut outbound_models = Vec::new();
        let mut revert_models = Vec::new();

        // First, prepare all the parent records
        for cctx in cctxs {
            let cctx_model = CrossChainTxEntity::ActiveModel {
                id: ActiveValue::NotSet,
                creator: ActiveValue::Set(cctx.creator.clone()),
                index: ActiveValue::Set(cctx.index.clone()),
                retries_number: ActiveValue::Set(0),
                processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
                zeta_fees: ActiveValue::Set(cctx.zeta_fees.clone()),
                relayed_message: ActiveValue::Set(Some(cctx.relayed_message.clone())),
                protocol_contract_version: ActiveValue::Set(
                    ProtocolContractVersion::try_from(cctx.protocol_contract_version.clone())
                        .map_err(|e| anyhow::anyhow!(e))?,
                ),
                last_status_update_timestamp: ActiveValue::Set(Utc::now().naive_utc()),
                root_id: ActiveValue::Set(None), //When querying we treat None as being your own root
                parent_id: ActiveValue::Set(None),
                depth: ActiveValue::Set(0),
                updated_by: ActiveValue::Set(job_id.to_string()),
            };
            cctx_models.push(cctx_model);
        }
        let indices = cctx_models
            .iter()
            .map(|cctx| cctx.index.as_ref().clone())
            .collect::<Vec<String>>();
        // Batch insert parent records
        let inserted_cctxs = CrossChainTxEntity::Entity::insert_many(cctx_models)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(CrossChainTxEntity::Column::Index)
                    .update_columns([
                        CrossChainTxEntity::Column::LastStatusUpdateTimestamp,
                        CrossChainTxEntity::Column::UpdatedBy,
                    ])
                    .to_owned(),
            )
            .exec_with_returning_many(self.db.as_ref())
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Batch insert cctxs failed: {}, indices: {}",
                    e,
                    indices.join(",")
                )
            })?;

        tracing::debug!("job_id: {}, inserted cctxs: {}", job_id, indices.join(","));
        // Create a map from index to id for quick lookup
        let index_to_id: std::collections::HashMap<String, i32> = inserted_cctxs
            .into_iter()
            .map(|cctx| (cctx.index, cctx.id))
            .collect();

        // Now prepare child records for each transaction
        for cctx in cctxs {
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
                            cctx.cctx_status
                                .last_update_timestamp
                                .parse::<i64>()
                                .unwrap_or(0),
                            0,
                        )
                        .ok_or(anyhow::anyhow!("Invalid timestamp"))?
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
                let inbound_model = InboundParamsEntity::ActiveModel {
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
                let revert_model = RevertOptionsEntity::ActiveModel {
                    id: ActiveValue::NotSet,
                    cross_chain_tx_id: ActiveValue::Set(tx_id),
                    revert_address: ActiveValue::Set(Some(
                        cctx.revert_options.revert_address.clone(),
                    )),
                    call_on_revert: ActiveValue::Set(cctx.revert_options.call_on_revert),
                    abort_address: ActiveValue::Set(Some(
                        cctx.revert_options.abort_address.clone(),
                    )),
                    revert_message: ActiveValue::Set(cctx.revert_options.revert_message.clone()),
                    revert_gas_limit: ActiveValue::Set(
                        cctx.revert_options.revert_gas_limit.clone(),
                    ),
                };
                revert_models.push(revert_model);
            }
        }

        // Batch insert all child records
        if !status_models.is_empty() {
            cctx_status::Entity::insert_many(status_models)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::column(cctx_status::Column::CrossChainTxId)
                        .update_columns([
                            cctx_status::Column::Status,
                            cctx_status::Column::StatusMessage,
                            cctx_status::Column::ErrorMessage,
                            cctx_status::Column::LastUpdateTimestamp,
                            cctx_status::Column::IsAbortRefunded,
                            cctx_status::Column::CreatedTimestamp,
                            cctx_status::Column::ErrorMessageRevert,
                            cctx_status::Column::ErrorMessageAbort,
                        ])
                        .to_owned(),
                )
                .exec(tx)
                .instrument(tracing::debug_span!("inserting cctx_status"))
                .await?;
        }

        if !inbound_models.is_empty() {
            InboundParamsEntity::Entity::insert_many(inbound_models)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::column(
                        InboundParamsEntity::Column::BallotIndex,
                    )
                    .update_columns([
                        InboundParamsEntity::Column::Status,
                        InboundParamsEntity::Column::ConfirmationMode,
                    ])
                    .to_owned(),
                )
                .exec(tx)
                .instrument(tracing::debug_span!("inserting inbound_params"))
                .await
                .map_err(|e| anyhow::anyhow!("Batch insert inbound_params failed: {}", e))?;
        }
        if !outbound_models.is_empty() {
            let hash_set = outbound_models
                .iter()
                .map(|outbound| outbound.hash.as_ref().clone())
                .collect::<HashSet<String>>();
            if hash_set.len() != outbound_models.len() {
                tracing::error!(
                    "Batch insert outbound_params failed: duplicate hashes job_id: {} indices: {}",
                    job_id,
                    indices.join(",")
                );
                return Err(anyhow::anyhow!(
                    "Batch insert outbound_params failed: duplicate hashes"
                ));
            }
            OutboundParamsEntity::Entity::insert_many(outbound_models)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::column(OutboundParamsEntity::Column::Hash)
                        .update_columns([
                            OutboundParamsEntity::Column::TxFinalizationStatus,
                            OutboundParamsEntity::Column::GasUsed,
                            OutboundParamsEntity::Column::EffectiveGasPrice,
                            OutboundParamsEntity::Column::EffectiveGasLimit,
                            OutboundParamsEntity::Column::TssNonce,
                            OutboundParamsEntity::Column::CallOptionsGasLimit,
                            OutboundParamsEntity::Column::CallOptionsIsArbitraryCall,
                            OutboundParamsEntity::Column::ConfirmationMode,
                        ])
                        .to_owned(),
                )
                .exec(tx)
                .instrument(tracing::debug_span!("inserting outbound_params"))
                .await
                .map_err(|e| anyhow::anyhow!("Batch insert outbound_params failed: {}", e))?;
        }
        if !revert_models.is_empty() {
            RevertOptionsEntity::Entity::insert_many(revert_models)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::column(
                        RevertOptionsEntity::Column::CrossChainTxId,
                    )
                    .update_columns([
                        RevertOptionsEntity::Column::RevertAddress,
                        RevertOptionsEntity::Column::CallOnRevert,
                        RevertOptionsEntity::Column::AbortAddress,
                        RevertOptionsEntity::Column::RevertMessage,
                        RevertOptionsEntity::Column::RevertGasLimit,
                    ])
                    .to_owned(),
                )
                .exec(tx)
                .instrument(tracing::debug_span!("inserting revert_options"))
                .await
                .map_err(|e| anyhow::anyhow!("Batch insert revert_options failed: {}", e))?;
        }

        Ok(())
    }

    #[instrument(,level="debug",skip(self,job_id,watermark), fields(watermark_id = %watermark.id))]
    pub async fn unlock_watermark(
        &self,
        watermark: watermark::Model,
        job_id: Uuid,
    ) -> anyhow::Result<()> {
        let res = watermark::Entity::update(watermark::ActiveModel {
            id: ActiveValue::Unchanged(watermark.id),
            processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
            updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            updated_by: ActiveValue::Set(job_id.to_string()),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark.id))
        .exec(self.db.as_ref())
        .await?;

        tracing::debug!("unlocked watermark: {:?}", res);
        Ok(())
    }

    #[instrument(,level="debug",skip_all)]
    pub async fn lock_watermark(
        &self,
        watermark: watermark::Model,
        job_id: Uuid,
    ) -> anyhow::Result<()> {
        watermark::Entity::update(watermark::ActiveModel {
            id: ActiveValue::Unchanged(watermark.id),
            processing_status: ActiveValue::Set(ProcessingStatus::Locked),
            updated_by: ActiveValue::Set(job_id.to_string()),
            updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark.id))
        .exec(self.db.as_ref())
        .await?;
        Ok(())
    }

    #[instrument(,level="debug",skip(self),fields(watermark_id = %watermark.id,job_id = %job_id))]
    pub async fn mark_watermark_as_done(
        &self,
        watermark: watermark::Model,
        job_id: Uuid,
    ) -> anyhow::Result<()> {
        watermark::Entity::update(watermark::ActiveModel {
            id: ActiveValue::Unchanged(watermark.id),
            processing_status: ActiveValue::Set(ProcessingStatus::Done),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark.id))
        .exec(self.db.as_ref())
        .await?;
        Ok(())
    }

    #[instrument(,level="debug",skip_all)]
    pub async fn unlock_cctx(&self, id: i32, job_id: Uuid) -> anyhow::Result<()> {
        CrossChainTxEntity::Entity::update(CrossChainTxEntity::ActiveModel {
            id: ActiveValue::Unchanged(id),
            processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
            updated_by: ActiveValue::Set(job_id.to_string()),
            last_status_update_timestamp: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        })
        .filter(CrossChainTxEntity::Column::Id.eq(id))
        .exec(self.db.as_ref())
        .await?;
        Ok(())
    }

    #[instrument(,level="debug",skip(self),fields(batch_id = %batch_id))]
    pub async fn query_failed_cctxs(&self, batch_id: Uuid) -> anyhow::Result<Vec<CctxShort>> {
        let statement = format!(
            r#"
        WITH cctxs AS (
            SELECT cctx.id
            FROM cross_chain_tx cctx
            JOIN cctx_status cs ON cctx.id = cs.cross_chain_tx_id
            WHERE cctx.processing_status = 'failed'::processing_status
            AND cctx.last_status_update_timestamp + INTERVAL '1 hour' * POWER(2, cctx.retries_number) < NOW() 
            ORDER BY cctx.last_status_update_timestamp ASC,cs.created_timestamp DESC
            LIMIT 100
        )
        UPDATE cross_chain_tx cctx
        SET processing_status = 'locked'::processing_status, last_status_update_timestamp = NOW(), retries_number = retries_number + 1
        WHERE id IN (SELECT id FROM cctxs)
        RETURNING id, index, root_id, depth, retries_number
        "#
        );

        let statement = Statement::from_sql_and_values(DbBackend::Postgres, statement, vec![]);

        let cctxs: Result<Vec<CctxShort>, sea_orm::DbErr> = self
            .db
            .query_all(statement)
            .await?
            .iter()
            .map(|r| {
                std::result::Result::Ok(CctxShort {
                    id: r.try_get_by_index(0)?,
                    index: r.try_get_by_index(1)?,
                    root_id: r.try_get_by_index(2)?,
                    depth: r.try_get_by_index(3)?,
                    retries_number: r.try_get_by_index(4)?,
                })
            })
            .collect::<Result<Vec<CctxShort>, sea_orm::DbErr>>();

        cctxs.map_err(|e| anyhow::anyhow!(e))
    }

    #[instrument(,level="debug",skip(self), fields(batch_id = %batch_id))]
    pub async fn query_cctxs_for_status_update(
        &self,
        batch_size: u32,
        batch_id: Uuid,
        polling_interval: u64,
    ) -> anyhow::Result<Vec<CctxShort>> {
        let statement = format!(
            r#"
        WITH cctxs AS (
            SELECT cctx.id
            FROM cross_chain_tx cctx
            JOIN cctx_status cs ON cctx.id = cs.cross_chain_tx_id
            WHERE cctx.processing_status = 'unlocked'::processing_status
            AND cctx.last_status_update_timestamp + INTERVAL '{polling_interval} milliseconds' * (POWER(2, cctx.retries_number) - 1) < NOW() 
            ORDER BY cctx.last_status_update_timestamp ASC,cs.created_timestamp DESC
            LIMIT {batch_size}
            FOR UPDATE SKIP LOCKED
        )
        UPDATE cross_chain_tx cctx
        SET processing_status = 'locked'::processing_status, last_status_update_timestamp = NOW(), retries_number = retries_number + 1
        WHERE id IN (SELECT id FROM cctxs)
        RETURNING id, index, root_id, depth, retries_number
        "#
        );

        let statement = Statement::from_sql_and_values(DbBackend::Postgres, statement, vec![]);

        let cctxs: Result<Vec<CctxShort>, sea_orm::DbErr> = self
            .db
            .query_all(statement)
            .await?
            .iter()
            .map(|r| {
                std::result::Result::Ok(CctxShort {
                    id: r.try_get_by_index(0)?,
                    index: r.try_get_by_index(1)?,
                    root_id: r.try_get_by_index(2)?,
                    depth: r.try_get_by_index(3)?,
                    retries_number: r.try_get_by_index(4)?,
                })
            })
            .collect::<Result<Vec<CctxShort>, sea_orm::DbErr>>();

        cctxs.map_err(|e| anyhow::anyhow!(e))
    }

    #[instrument(,level="debug",skip(self,tx),fields(cctx_id = %cctx_id))]
    pub async fn mark_cctx_tree_processed(
        &self,
        cctx_id: i32,
        job_id: Uuid,
        cctx_index: &str,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        CrossChainTxEntity::Entity::update(CrossChainTxEntity::ActiveModel {
            id: ActiveValue::Set(cctx_id),
            processing_status: ActiveValue::Set(ProcessingStatus::Done),
            last_status_update_timestamp: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            updated_by: ActiveValue::Set(job_id.to_string()),
            ..Default::default()
        })
        .filter(CrossChainTxEntity::Column::Id.eq(cctx_id))
        .exec(tx)
        .await?;
        Ok(())
    }

    #[instrument(,level="debug",skip(self,tx),fields(cctx_id = %cctx_id, job_id = %job_id))]
    pub async fn update_last_status_update_timestamp(
        &self,
        cctx_id: i32,
        job_id: Uuid,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        CrossChainTxEntity::Entity::update(CrossChainTxEntity::ActiveModel {
            id: ActiveValue::Set(cctx_id),
            last_status_update_timestamp: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
            retries_number: ActiveValue::Set(0),
            updated_by: ActiveValue::Set(job_id.to_string()),
            ..Default::default()
        })
        .filter(CrossChainTxEntity::Column::Id.eq(cctx_id))
        .exec(tx)
        .await?;
        Ok(())
    }

    #[instrument(,level="debug",skip_all)]
    pub async fn update_cctx_status(
        &self,
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
            .await?
        {
            cctx_status::Entity::update(cctx_status::ActiveModel {
                    id: ActiveValue::Set(cctx_status_row.id),
                    status: ActiveValue::Set(CctxStatusStatus::try_from(fetched_cctx.cctx_status.status.clone())
                        .map_err(|e| anyhow::anyhow!(e))?
                    ),
                    last_update_timestamp: ActiveValue::Set(
                        chrono::DateTime::from_timestamp(
                            fetched_cctx.cctx_status.last_update_timestamp.parse::<i64>().unwrap_or(0),
                            0,
                        ).ok_or(anyhow::anyhow!("Invalid timestamp"))?
                        .naive_utc(),
                    ),
                    ..Default::default()
                })
                .filter(cctx_status::Column::Id.eq(cctx_status_row.id))
                .exec(self.db.as_ref())
                .instrument(tracing::debug_span!("updating cctx_status", new_status = %fetched_cctx.cctx_status.status))
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
    #[instrument(,level="debug",skip(self, cctx, tx),fields(index = %cctx.index, job_id = %job_id, root_id = %root_id, parent_id = %parent_id, depth = %depth))]
    pub async fn import_related_cctx(
        &self,
        cctx: CrossChainTx,
        job_id: Uuid,
        root_id: i32,
        parent_id: i32,
        depth: i32,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        // Insert main cross_chain_tx record
        let index = cctx.index.clone();
        let cctx_model = CrossChainTxEntity::ActiveModel {
            id: ActiveValue::NotSet,
            creator: ActiveValue::Set(cctx.creator),
            index: ActiveValue::Set(cctx.index),
            retries_number: ActiveValue::Set(0),
            processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
            zeta_fees: ActiveValue::Set(cctx.zeta_fees),
            relayed_message: ActiveValue::Set(Some(cctx.relayed_message)),
            protocol_contract_version: ActiveValue::Set(
                ProtocolContractVersion::try_from(cctx.protocol_contract_version)
                    .map_err(|e| anyhow::anyhow!(e))?,
            ),
            last_status_update_timestamp: ActiveValue::Set(Utc::now().naive_utc()),
            root_id: ActiveValue::Set(Some(root_id)),
            parent_id: ActiveValue::Set(Some(parent_id)),
            depth: ActiveValue::Set(depth),
            updated_by: ActiveValue::Set(job_id.to_string()),
        };

        let tx_result = CrossChainTxEntity::Entity::insert(cctx_model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(CrossChainTxEntity::Column::Index)
                    .do_nothing()
                    .to_owned(),
            )
            .exec(tx)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to import related cctx: {}", e))?;

        // Get the inserted tx id
        let tx_id = tx_result.last_insert_id;

        // Insert cctx_status
        let status_model = cctx_status::ActiveModel {
            id: ActiveValue::NotSet,
            cross_chain_tx_id: ActiveValue::Set(tx_id),
            status: ActiveValue::Set(
                CctxStatusStatus::try_from(cctx.cctx_status.status.clone())
                    .map_err(|e| anyhow::anyhow!(e))?,
            ),
            status_message: ActiveValue::Set(Some(sanitize_string(
                cctx.cctx_status.status_message,
            ))),
            error_message: ActiveValue::Set(Some(sanitize_string(cctx.cctx_status.error_message))),
            last_update_timestamp: ActiveValue::Set(
                // parse NaiveDateTime from epoch seconds
                chrono::DateTime::from_timestamp(
                    cctx.cctx_status
                        .last_update_timestamp
                        .parse::<i64>()
                        .unwrap_or(0),
                    0,
                )
                .ok_or(anyhow::anyhow!("Invalid timestamp"))?
                .naive_utc(),
            ),
            is_abort_refunded: ActiveValue::Set(cctx.cctx_status.is_abort_refunded),
            created_timestamp: ActiveValue::Set(cctx.cctx_status.created_timestamp.parse::<i64>()?),
            error_message_revert: ActiveValue::Set(Some(sanitize_string(
                cctx.cctx_status.error_message_revert,
            ))),
            error_message_abort: ActiveValue::Set(Some(sanitize_string(
                cctx.cctx_status.error_message_abort,
            ))),
        };
        cctx_status::Entity::insert(status_model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(cctx_status::Column::CrossChainTxId)
                    .do_nothing()
                    .to_owned(),
            )
            .exec(tx)
            .instrument(
                tracing::debug_span!("inserting cctx_status", index = %index, job_id = %job_id),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to import related cctx_status: {}", e))?;

        // Insert inbound_params
        let inbound_model = InboundParamsEntity::ActiveModel {
            id: ActiveValue::NotSet,
            cross_chain_tx_id: ActiveValue::Set(tx_id),
            sender: ActiveValue::Set(cctx.inbound_params.sender),
            sender_chain_id: ActiveValue::Set(cctx.inbound_params.sender_chain_id),
            tx_origin: ActiveValue::Set(cctx.inbound_params.tx_origin),
            coin_type: ActiveValue::Set(
                CoinType::try_from(cctx.inbound_params.coin_type)
                    .map_err(|e| anyhow::anyhow!(e))?,
            ),
            asset: ActiveValue::Set(Some(cctx.inbound_params.asset)),
            amount: ActiveValue::Set(cctx.inbound_params.amount),
            observed_hash: ActiveValue::Set(cctx.inbound_params.observed_hash),
            observed_external_height: ActiveValue::Set(
                cctx.inbound_params.observed_external_height,
            ),
            ballot_index: ActiveValue::Set(cctx.inbound_params.ballot_index),
            finalized_zeta_height: ActiveValue::Set(cctx.inbound_params.finalized_zeta_height),
            tx_finalization_status: ActiveValue::Set(
                TxFinalizationStatus::try_from(cctx.inbound_params.tx_finalization_status)
                    .map_err(|e| anyhow::anyhow!(e))?,
            ),
            is_cross_chain_call: ActiveValue::Set(cctx.inbound_params.is_cross_chain_call),
            status: ActiveValue::Set(
                InboundStatus::try_from(cctx.inbound_params.status)
                    .map_err(|e| anyhow::anyhow!(e))?,
            ),
            confirmation_mode: ActiveValue::Set(
                ConfirmationMode::try_from(cctx.inbound_params.confirmation_mode)
                    .map_err(|e| anyhow::anyhow!(e))?,
            ),
        };
        InboundParamsEntity::Entity::insert(inbound_model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(InboundParamsEntity::Column::ObservedHash)
                    .do_nothing()
                    .to_owned(),
            )
            .exec(tx)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to import related inbound_params: {}", e))?;

        // Insert outbound_params
        for outbound in cctx.outbound_params {
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
                .on_conflict(
                    sea_orm::sea_query::OnConflict::column(OutboundParamsEntity::Column::Hash)
                        .do_nothing()
                        .to_owned(),
                )
                .exec(tx)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to import related outbound_params: {}", e))?;
        }

        // Insert revert_options
        let revert_model = RevertOptionsEntity::ActiveModel {
            id: ActiveValue::NotSet,
            cross_chain_tx_id: ActiveValue::Set(tx_id),
            revert_address: ActiveValue::Set(Some(cctx.revert_options.revert_address)),
            call_on_revert: ActiveValue::Set(cctx.revert_options.call_on_revert),
            abort_address: ActiveValue::Set(Some(cctx.revert_options.abort_address)),
            revert_message: ActiveValue::Set(cctx.revert_options.revert_message),
            revert_gas_limit: ActiveValue::Set(cctx.revert_options.revert_gas_limit),
        };
        RevertOptionsEntity::Entity::insert(revert_model)
            .exec(tx)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to import related revert_options: {}", e))?;

        Ok(())
    }

    pub async fn get_complete_cctx(&self, index: String) -> anyhow::Result<Option<CompleteCctx>> {
        // Single query to get all data with JOINs
        let sql = r#"
        SELECT 
            -- CrossChainTx fields
            cctx.id as cctx_id,--0
            cctx.creator,--1
            cctx.index,--2
            cctx.zeta_fees,--3
            cctx.retries_number,--4
            cctx.processing_status::text,--5
            cctx.relayed_message,--6
            cctx.last_status_update_timestamp,--7
            cctx.protocol_contract_version::text,--8
            cctx.root_id,--9
            cctx.parent_id,--10
            cctx.depth,--11
            cctx.updated_by,--12
            -- CctxStatus fields
            cs.id as status_id,--13
            cs.cross_chain_tx_id as status_cross_chain_tx_id,--14
            cs.status::text,--15
            cs.status_message::text,--16
            cs.error_message,--17
            cs.last_update_timestamp,--18
            cs.is_abort_refunded,--19
            cs.created_timestamp,--20
            cs.error_message_revert,--21
            cs.error_message_abort,--22
            -- InboundParams fields
            ip.id as inbound_id,--23
            ip.cross_chain_tx_id as inbound_cross_chain_tx_id,--24
            ip.sender,--25
            ip.sender_chain_id,--26
            ip.tx_origin,--27
            ip.coin_type::text,--28
            ip.asset,--29
            ip.amount,--30
            ip.observed_hash,--31
            ip.observed_external_height,--32
            ip.ballot_index,--33
            ip.finalized_zeta_height,--34
            ip.tx_finalization_status::text,--35
            ip.is_cross_chain_call,--36
            ip.status::text as inbound_status,--37
            ip.confirmation_mode::text as inbound_confirmation_mode,--38
            -- RevertOptions fields
            ro.id as revert_id,--39
            ro.cross_chain_tx_id as revert_cross_chain_tx_id,--40
            ro.revert_address,--41
            ro.call_on_revert,--42
            ro.abort_address,--43
            ro.revert_message,--44
            ro.revert_gas_limit--45
        FROM cross_chain_tx cctx
        LEFT JOIN cctx_status cs ON cctx.id = cs.cross_chain_tx_id
        LEFT JOIN inbound_params ip ON cctx.id = ip.cross_chain_tx_id
        LEFT JOIN revert_options ro ON cctx.id = ro.cross_chain_tx_id
        WHERE cctx.index = $1
        "#;

        let statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            vec![sea_orm::Value::String(Some(Box::new(index.clone())))],
        );

        let cctx_rows = self.db.query_all(statement).await?;

        if cctx_rows.is_empty() {
            return Ok(None);
        }

        // Get the first row for main CCTX, status, inbound, and revert data
        let cctx_row = &cctx_rows[0];

        // Check if required entities exist
        let status_id: Option<i32> = cctx_row
            .try_get_by_index(13)
            .map_err(|e| anyhow::anyhow!("cctx_row status_id: {}", e))?;
        let inbound_id: Option<i32> = cctx_row
            .try_get_by_index(23)
            .map_err(|e| anyhow::anyhow!("cctx_row inbound_id: {}", e))?;
        let revert_id: Option<i32> = cctx_row
            .try_get_by_index(39)
            .map_err(|e| anyhow::anyhow!("cctx_row revert_id: {}", e))?;

        if status_id.is_none() {
            return Err(anyhow::anyhow!(
                "CCTX status not found for index: {}",
                index
            ));
        }
        if inbound_id.is_none() {
            return Err(anyhow::anyhow!(
                "Inbound params not found for index: {}",
                index
            ));
        }
        if revert_id.is_none() {
            return Err(anyhow::anyhow!(
                "Revert options not found for index: {}",
                index
            ));
        }

        // Build CrossChainTx
        let cctx = CrossChainTxEntity::Model {
            id: cctx_row
                .try_get_by_index(0)
                .map_err(|e| anyhow::anyhow!("cctx_row id: {}", e))?,
            creator: cctx_row
                .try_get_by_index(1)
                .map_err(|e| anyhow::anyhow!("cctx_row creator: {}", e))?,
            index: cctx_row
                .try_get_by_index(2)
                .map_err(|e| anyhow::anyhow!("cctx_row index: {}", e))?,
            zeta_fees: cctx_row
                .try_get_by_index(3)
                .map_err(|e| anyhow::anyhow!("cctx_row zeta_fees: {}", e))?,
            retries_number: cctx_row
                .try_get_by_index(4)
                .map_err(|e| anyhow::anyhow!("cctx_row retries_number: {}", e))?,
            processing_status: ProcessingStatus::try_from(cctx_row.try_get_by_index::<String>(5)?)
                .map_err(|_| sea_orm::DbErr::Custom("Invalid processing_status".to_string()))?,
            relayed_message: cctx_row
                .try_get_by_index(6)
                .map_err(|e| anyhow::anyhow!("cctx_row relayed_message: {}", e))?,
            last_status_update_timestamp: cctx_row
                .try_get_by_index(7)
                .map_err(|e| anyhow::anyhow!("cctx_row last_status_update_timestamp: {}", e))?,
            protocol_contract_version: ProtocolContractVersion::try_from(
                cctx_row.try_get_by_index::<String>(8)?,
            )
            .map_err(|_| sea_orm::DbErr::Custom("Invalid protocol_contract_version".to_string()))?,
            root_id: cctx_row
                .try_get_by_index(9)
                .map_err(|e| anyhow::anyhow!("cctx_row root_id: {}", e))?,
            parent_id: cctx_row
                .try_get_by_index(10)
                .map_err(|e| anyhow::anyhow!("cctx_row parent_id: {}", e))?,
            depth: cctx_row
                .try_get_by_index(11)
                .map_err(|e| anyhow::anyhow!("cctx_row depth: {}", e))?,
            updated_by: cctx_row
                .try_get_by_index(12)
                .map_err(|e| anyhow::anyhow!("cctx_row updated_by: {}", e))?,
        };

        // Build CctxStatus
        let status = cctx_status::Model {
            id: cctx_row
                .try_get_by_index(13)
                .map_err(|e| anyhow::anyhow!("cctx_row id: {}", e))?,
            cross_chain_tx_id: cctx_row
                .try_get_by_index(14)
                .map_err(|e| anyhow::anyhow!("cctx_row cross_chain_tx_id: {}", e))?,
            status: CctxStatusStatus::try_from(cctx_row.try_get_by_index::<String>(15)?)
                .map_err(|_| sea_orm::DbErr::Custom("Invalid status".to_string()))?,
            status_message: cctx_row
                .try_get_by_index(16)
                .map_err(|e| anyhow::anyhow!("cctx_row status_message: {}", e))?,
            error_message: cctx_row
                .try_get_by_index(17)
                .map_err(|e| anyhow::anyhow!("cctx_row error_message: {}", e))?,
            last_update_timestamp: cctx_row
                .try_get_by_index(18)
                .map_err(|e| anyhow::anyhow!("cctx_row last_update_timestamp: {}", e))?,
            is_abort_refunded: cctx_row
                .try_get_by_index(19)
                .map_err(|e| anyhow::anyhow!("cctx_row is_abort_refunded: {}", e))?,
            created_timestamp: cctx_row
                .try_get_by_index(20)
                .map_err(|e| anyhow::anyhow!("cctx_row created_timestamp: {}", e))?,
            error_message_revert: cctx_row
                .try_get_by_index(21)
                .map_err(|e| anyhow::anyhow!("cctx_row error_message_revert: {}", e))?,
            error_message_abort: cctx_row
                .try_get_by_index(22)
                .map_err(|e| anyhow::anyhow!("cctx_row error_message_abort: {}", e))?,
        };

        // Build InboundParams
        let inbound = InboundParamsEntity::Model {
            id: cctx_row
                .try_get_by_index(23)
                .map_err(|e| anyhow::anyhow!("cctx_row id: {}", e))?,
            cross_chain_tx_id: cctx_row
                .try_get_by_index(24)
                .map_err(|e| anyhow::anyhow!("cctx_row cross_chain_tx_id: {}", e))?,
            sender: cctx_row
                .try_get_by_index(25)
                .map_err(|e| anyhow::anyhow!("cctx_row sender: {}", e))?,
            sender_chain_id: cctx_row
                .try_get_by_index(26)
                .map_err(|e| anyhow::anyhow!("cctx_row sender_chain_id: {}", e))?,
            tx_origin: cctx_row
                .try_get_by_index(27)
                .map_err(|e| anyhow::anyhow!("cctx_row tx_origin: {}", e))?,
            coin_type: CoinType::try_from(cctx_row.try_get_by_index::<String>(28)?)
                .map_err(|_| sea_orm::DbErr::Custom("Invalid coin_type".to_string()))?,
            asset: cctx_row
                .try_get_by_index(29)
                .map_err(|e| anyhow::anyhow!("cctx_row asset: {}", e))?,
            amount: cctx_row
                .try_get_by_index(30)
                .map_err(|e| anyhow::anyhow!("cctx_row amount: {}", e))?,
            observed_hash: cctx_row
                .try_get_by_index(31)
                .map_err(|e| anyhow::anyhow!("cctx_row observed_hash: {}", e))?,
            observed_external_height: cctx_row
                .try_get_by_index(32)
                .map_err(|e| anyhow::anyhow!("cctx_row observed_external_height: {}", e))?,
            ballot_index: cctx_row
                .try_get_by_index(33)
                .map_err(|e| anyhow::anyhow!("cctx_row ballot_index: {}", e))?,
            finalized_zeta_height: cctx_row
                .try_get_by_index(34)
                .map_err(|e| anyhow::anyhow!("cctx_row finalized_zeta_height: {}", e))?,
            tx_finalization_status: TxFinalizationStatus::try_from(
                cctx_row.try_get_by_index::<String>(35)?,
            )
            .map_err(|_| sea_orm::DbErr::Custom("Invalid tx_finalization_status".to_string()))?,
            is_cross_chain_call: cctx_row
                .try_get_by_index(36)
                .map_err(|e| anyhow::anyhow!("cctx_row is_cross_chain_call: {}", e))?,
            status: InboundStatus::try_from(cctx_row.try_get_by_index::<String>(37)?)
                .map_err(|_| sea_orm::DbErr::Custom("Invalid inbound_status".to_string()))?,
            confirmation_mode: ConfirmationMode::try_from(cctx_row.try_get_by_index::<String>(38)?)
                .map_err(|_| {
                    sea_orm::DbErr::Custom("Invalid inbound_confirmation_mode".to_string())
                })?,
        };

        // Build RevertOptions
        let revert = RevertOptionsEntity::Model {
            id: cctx_row
                .try_get_by_index(39)
                .map_err(|e| anyhow::anyhow!("cctx_row id: {}", e))?,
            cross_chain_tx_id: cctx_row
                .try_get_by_index(40)
                .map_err(|e| anyhow::anyhow!("cctx_row cross_chain_tx_id: {}", e))?,
            revert_address: cctx_row
                .try_get_by_index(41)
                .map_err(|e| anyhow::anyhow!("cctx_row revert_address: {}", e))?,
            call_on_revert: cctx_row
                .try_get_by_index(42)
                .map_err(|e| anyhow::anyhow!("cctx_row call_on_revert: {}", e))?,
            abort_address: cctx_row
                .try_get_by_index(43)
                .map_err(|e| anyhow::anyhow!("cctx_row abort_address: {}", e))?,
            revert_message: cctx_row
                .try_get_by_index(44)
                .map_err(|e| anyhow::anyhow!("cctx_row revert_message: {}", e))?,
            revert_gas_limit: cctx_row
                .try_get_by_index(45)
                .map_err(|e| anyhow::anyhow!("cctx_row revert_gas_limit: {}", e))?,
        };

        // Now get outbound params (can be multiple, so we need a separate query)
        let outbounds_sql = r#"
        SELECT 
            id,
            cross_chain_tx_id,
            receiver,
            receiver_chain_id,
            coin_type::text,
            amount,
            tss_nonce,
            gas_limit,
            gas_price,
            gas_priority_fee,
            hash,
            ballot_index,
            observed_external_height,
            gas_used,
            effective_gas_price,
            effective_gas_limit,
            tss_pubkey,
            tx_finalization_status::text,
            call_options_gas_limit,
            call_options_is_arbitrary_call,
            confirmation_mode::text
        FROM outbound_params
        WHERE cross_chain_tx_id = $1
        "#;

        let outbounds_statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            outbounds_sql,
            vec![sea_orm::Value::Int(Some(cctx.id))],
        );

        let outbounds_rows = self.db.query_all(outbounds_statement).await?;
        let mut outbounds = Vec::new();

        for outbound_row in outbounds_rows {
            outbounds.push(OutboundParamsEntity::Model {
                id: outbound_row.try_get_by_index(0)?,
                cross_chain_tx_id: outbound_row
                    .try_get_by_index(1)
                    .map_err(|e| anyhow::anyhow!("outbound_row cross_chain_tx_id: {}", e))?,
                receiver: outbound_row
                    .try_get_by_index(2)
                    .map_err(|e| anyhow::anyhow!("outbound_row receiver: {}", e))?,
                receiver_chain_id: outbound_row
                    .try_get_by_index(3)
                    .map_err(|e| anyhow::anyhow!("outbound_row receiver_chain_id: {}", e))?,
                coin_type: CoinType::try_from(outbound_row.try_get_by_index::<String>(4)?)
                    .map_err(|_| {
                        sea_orm::DbErr::Custom(
                            "outbound_row:Invalid outbound coin_type".to_string(),
                        )
                    })?,
                amount: outbound_row
                    .try_get_by_index(5)
                    .map_err(|e| anyhow::anyhow!("outbound_row amount: {}", e))?,
                tss_nonce: outbound_row
                    .try_get_by_index(6)
                    .map_err(|e| anyhow::anyhow!("outbound_row tss_nonce: {}", e))?,
                gas_limit: outbound_row
                    .try_get_by_index(7)
                    .map_err(|e| anyhow::anyhow!("outbound_row gas_limit: {}", e))?,
                gas_price: outbound_row
                    .try_get_by_index(8)
                    .map_err(|e| anyhow::anyhow!("outbound_row gas_price: {}", e))?,
                gas_priority_fee: outbound_row
                    .try_get_by_index(9)
                    .map_err(|e| anyhow::anyhow!("outbound_row gas_priority_fee: {}", e))?,
                hash: outbound_row
                    .try_get_by_index(10)
                    .map_err(|e| anyhow::anyhow!("outbound_row hash: {}", e))?,
                ballot_index: outbound_row
                    .try_get_by_index(11)
                    .map_err(|e| anyhow::anyhow!("outbound_row ballot_index: {}", e))?,
                observed_external_height: outbound_row
                    .try_get_by_index(12)
                    .map_err(|e| anyhow::anyhow!("outbound_row observed_external_height: {}", e))?,
                gas_used: outbound_row
                    .try_get_by_index(13)
                    .map_err(|e| anyhow::anyhow!("outbound_row gas_used: {}", e))?,
                effective_gas_price: outbound_row
                    .try_get_by_index(14)
                    .map_err(|e| anyhow::anyhow!("outbound_row effective_gas_price: {}", e))?,
                effective_gas_limit: outbound_row
                    .try_get_by_index(15)
                    .map_err(|e| anyhow::anyhow!("outbound_row effective_gas_limit: {}", e))?,
                tss_pubkey: outbound_row
                    .try_get_by_index(16)
                    .map_err(|e| anyhow::anyhow!("outbound_row tss_pubkey: {}", e))?,
                tx_finalization_status: TxFinalizationStatus::try_from(
                    outbound_row.try_get_by_index::<String>(17)?,
                )
                .map_err(|_| {
                    sea_orm::DbErr::Custom(
                        "outbound_row:Invalid outbound tx_finalization_status".to_string(),
                    )
                })?,
                call_options_gas_limit: outbound_row
                    .try_get_by_index(18)
                    .map_err(|e| anyhow::anyhow!("outbound_row call_options_gas_limit: {}", e))?,
                call_options_is_arbitrary_call: outbound_row.try_get_by_index(19).map_err(|e| {
                    anyhow::anyhow!("outbound_row call_options_is_arbitrary_call: {}", e)
                })?,
                confirmation_mode: ConfirmationMode::try_from(
                    outbound_row.try_get_by_index::<String>(20)?,
                )
                .map_err(|_| {
                    sea_orm::DbErr::Custom("Invalid outbound confirmation_mode".to_string())
                })?,
            });
        }

        let related_sql = r#"
            select
            related.id, --0
            related.index, --1
            related.depth, --2
            ip.sender_chain_id, --3
            cs.status::text, --4
            ip.amount, --5
            ip.coin_type::text, --6
            ip.asset --7
        from
            cross_chain_tx cctx
            join cross_chain_tx root on COALESCE(cctx.root_id, cctx.id) = root.id
            join cross_chain_tx related on COALESCE(related.root_id, related.id) = root.id
            and related.id != cctx.id
            join cctx_status cs on related.id = cs.cross_chain_tx_id
            join inbound_params ip on related.id = ip.cross_chain_tx_id
            
        where
            cctx.index = $1
        order by 2
"#;

        let related_statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            related_sql,
            vec![sea_orm::Value::String(Some(Box::new(index)))],
        );

        let related_rows = self.db.query_all(related_statement).await?;
        let mut related = Vec::new();

        for related_row in related_rows {
            let related_id: i32 = related_row.try_get_by_index(0)?;
            let outbound_params = OutboundParamsEntity::Entity::find()
                .filter(OutboundParamsEntity::Column::CrossChainTxId.eq(related_id))
                .all(self.db.as_ref())
                .await?
                .into_iter()
                .map(|x| RelatedOutboundParams {
                    amount: x.amount,
                    chain_id: x.receiver_chain_id,
                    coin_type: x.coin_type.to_string(),
                })
                .collect();

            related.push(RelatedCctx {
                index: related_row
                    .try_get_by_index(1)
                    .map_err(|e| anyhow::anyhow!("related_row index: {}", e))?,
                depth: related_row
                    .try_get_by_index(2)
                    .map_err(|e| anyhow::anyhow!("related_row depth: {}", e))?,
                source_chain_id: related_row
                    .try_get_by_index(3)
                    .map_err(|e| anyhow::anyhow!("related_row source_chain_id: {}", e))?,
                status: related_row
                    .try_get_by_index(4)
                    .map_err(|e| anyhow::anyhow!("related_row status: {}", e))?,
                inbound_amount: related_row
                    .try_get_by_index(5)
                    .map_err(|e| anyhow::anyhow!("related_row inbound_amount: {}", e))?,
                inbound_coin_type: related_row
                    .try_get_by_index(6)
                    .map_err(|e| anyhow::anyhow!("related_row inbound_coin_type: {}", e))?,
                inbound_asset: related_row
                    .try_get_by_index(7)
                    .map_err(|e| anyhow::anyhow!("related_row inbound_asset: {}", e))?,
                outbound_params,
            });
        }

        Ok(Some(CompleteCctx {
            cctx,
            status,
            inbound,
            outbounds,
            revert,
            related,
        }))
    }

    pub async fn list_cctxs(
        &self,
        limit: i64,
        offset: i64,
        status_filter: Option<String>,
    ) -> anyhow::Result<Vec<CctxListItem>> {
        let mut sql = String::from(
            r#"
        SELECT 
        cctx.index, --0
        cs.status::text, --1
        cs.last_update_timestamp, --2
        ip.amount, --3
        ip.sender_chain_id, --4
        string_agg(op.receiver_chain_id::text, ',') AS receiver_chain_id,
        MAX(cs.created_timestamp) AS created_ts
        FROM cross_chain_tx cctx
        INNER JOIN cctx_status cs ON cctx.id = cs.cross_chain_tx_id
        INNER JOIN inbound_params ip ON cctx.id = ip.cross_chain_tx_id
        INNER JOIN outbound_params op ON cctx.id = op.cross_chain_tx_id
        "#,
        );

        let mut params: Vec<sea_orm::Value> = Vec::new();
        let mut param_count = 0;

        // Add status filter if provided
        if let Some(status) = status_filter {
            param_count += 1;
            sql.push_str(&format!(" WHERE cs.status::text = ${}", param_count));
            params.push(sea_orm::Value::String(Some(Box::new(status))));
        }

        // Aggregate multiple outbound_params rows into a single row per CCTX
        sql.push_str(" GROUP BY cctx.index, cs.status, ip.sender_chain_id, cs.last_update_timestamp ");

        // Add ordering and pagination
        param_count += 1;
        sql.push_str(&format!(" ORDER BY created_ts DESC LIMIT ${}", param_count));
        params.push(sea_orm::Value::BigInt(Some(limit)));

        param_count += 1;
        sql.push_str(&format!(" OFFSET ${}", param_count));
        params.push(sea_orm::Value::BigInt(Some(offset)));

        let statement = Statement::from_sql_and_values(DbBackend::Postgres, sql.clone(), params);

        let rows = self.db.query_all(statement).await?;

        let mut items = Vec::new();
        for row in rows {
            // Read the status enum directly from the database
            let index =row.try_get_by_index(0).map_err(|e| anyhow::anyhow!("index: {}", e))?;
            let status = row.try_get_by_index::<String>(1).map_err(|e| anyhow::anyhow!("status: {}", e))?;
            let last_update_timestamp = row.try_get_by_index::<NaiveDateTime>(2)
                .map_err(|e| anyhow::anyhow!("last_update_timestamp: {}", e))?;
            let amount = row.try_get_by_index(3).map_err(|e| anyhow::anyhow!("amount: {}", e))?;
            let source_chain_id = row.try_get_by_index(4).map_err(|e| anyhow::anyhow!("source_chain_id: {}", e))?;
            let target_chain_id = row.try_get_by_index(5).map_err(|e| anyhow::anyhow!("target_chain_id: {}", e))?;

            items.push(CctxListItem {
                index,
                status,
                amount,
                last_update_timestamp,
                source_chain_id,
                target_chain_id,
            });
        }

        tracing::info!("items: {:?}", items);

        Ok(items)
    }
}
