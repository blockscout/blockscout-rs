use std::sync::Arc;

use crate::models::{self, CctxShort, CrossChainTx, Filters, SyncProgress, Token};
use anyhow::Ok;
use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{
    ActiveValue, DatabaseConnection, DatabaseTransaction, DbBackend, Statement, TransactionTrait,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use sea_orm::{ConnectionTrait, QueryOrder};
use tracing::{instrument, Instrument};
use uuid::Uuid;
use zetachain_cctx_entity::sea_orm_active_enums::ProcessingStatus;
use zetachain_cctx_entity::{
    cctx_status, cross_chain_tx as CrossChainTxEntity, inbound_params as InboundParamsEntity,
    outbound_params as OutboundParamsEntity, revert_options as RevertOptionsEntity,
    sea_orm_active_enums::{
        CctxStatusStatus, CoinType as DBCoinType, ConfirmationMode, InboundStatus, Kind,
        ProtocolContractVersion, TxFinalizationStatus,
    },
    token as TokenEntity, watermark,
};
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::{
    CallOptions as CallOptionsProto, CctxListItem as CctxListItemProto,
    CctxStatus as CctxStatusProto, CctxStatusReduced as CctxStatusReducedProto,
    CoinType as CoinTypeProto, ConfirmationMode as ConfirmationModeProto,
    CrossChainTx as CrossChainTxProto, Direction, InboundParams as InboundParamsProto,
    InboundStatus as InboundStatusProto, ListCctxsResponse, OutboundParams as OutboundParamsProto,
    Pagination, RelatedCctx as RelatedCctxProto,
    RelatedOutboundParams as RelatedOutboundParamsProto, RevertOptions as RevertOptionsProto,
    Status as StatusProto, Token as TokenProto, TxFinalizationStatus as TxFinalizationStatusProto,
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

pub fn reduce_status(status: CctxStatusProto) -> CctxStatusReducedProto {
    match status {
        CctxStatusProto::PendingOutbound
        | CctxStatusProto::PendingInbound
        | CctxStatusProto::PendingRevert => CctxStatusReducedProto::Pending,
        CctxStatusProto::OutboundMined => CctxStatusReducedProto::Success,
        CctxStatusProto::Aborted | CctxStatusProto::Reverted => CctxStatusReducedProto::Failed,
    }
}

impl ZetachainCctxDatabase {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
    #[instrument(level = "debug", skip_all)]
    pub async fn mark_watermark_as_failed(&self, watermark_id: i32) -> anyhow::Result<()> {
        watermark::Entity::update(watermark::ActiveModel {
            id: ActiveValue::Set(watermark_id),
            processing_status: ActiveValue::Set(ProcessingStatus::Failed),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark_id))
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

    #[instrument(level = "trace", skip_all)]
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

    #[instrument(level = "trace", skip_all)]
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
    #[instrument(level = "trace", skip_all,fields(child_index = %child_index, child_id = %child_id, root_id = %root_id, parent_id = %parent_id, depth = %depth))]
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

    #[instrument(level = "trace", skip_all)]
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

    #[instrument(level = "info", skip_all)]
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
                updated_by: ActiveValue::Set("setup".to_string()),
                id: ActiveValue::NotSet,
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

        let token_watermark = watermark::Entity::find()
            .filter(watermark::Column::Kind.eq(Kind::Token))
            .one(self.db.as_ref())
            .await?;
        if token_watermark.is_none() {
            tracing::debug!("inserting token watermark");
            watermark::Entity::insert(watermark::ActiveModel {
                kind: ActiveValue::Set(Kind::Token),
                processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
                pointer: ActiveValue::Set("MH==".to_string()), //0 in base64
                created_at: ActiveValue::Set(Utc::now().naive_utc()),
                updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                updated_by: ActiveValue::Set("setup".to_string()),
                id: ActiveValue::NotSet,
                ..Default::default()
            })
            .exec(self.db.as_ref())
            .await?;
        } else {
            tracing::debug!(
                "token watermark already exist, pointer: {}",
                token_watermark.unwrap().pointer
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

    #[instrument(skip(self,tx,watermark_id),level="debug",fields(watermark_id = %watermark_id, new_pointer = %new_pointer))]
    pub async fn move_watermark(
        &self,
        watermark_id: i32,
        new_pointer: &str,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        watermark::Entity::update(watermark::ActiveModel {
            id: ActiveValue::Set(watermark_id),
            processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
            pointer: ActiveValue::Set(new_pointer.to_owned()),
            updated_at: ActiveValue::Set(Utc::now().naive_utc()),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark_id))
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
    pub async fn import_cctxs_and_move_watermark(
        self: &ZetachainCctxDatabase,
        job_id: Uuid,
        cctxs: Vec<CrossChainTx>,
        next_key: &str,
        realtime_threshold: i64,
        watermark_id: i32,
    ) -> anyhow::Result<(ProcessingStatus, Vec<CctxListItemProto>)> {
        let earliest_fetched = cctxs.last().unwrap();
        let earliest_exists = earliest_fetched
            .cctx_status
            .last_update_timestamp
            .parse::<i64>()
            .unwrap_or(0)
            < Utc::now().timestamp() - realtime_threshold;

        tracing::debug!("last synced cctx is absent, inserting a batch and moving watermark");
        let tx = self.db.begin().await?;
        let inserted = self.batch_insert_transactions(job_id, &cctxs, &tx).await?;
        self.move_watermark(watermark_id, next_key, &tx).await?;
        tx.commit().await?;
        if earliest_exists {
            Ok((ProcessingStatus::Done, inserted))
        } else {
            Ok((ProcessingStatus::Unlocked, inserted))
        }
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn update_watermark(
        &self,
        watermark_id: i32,
        pointer: &str,
        upper_bound_timestamp: Option<NaiveDateTime>,
        status: ProcessingStatus,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        watermark::Entity::update(watermark::ActiveModel {
            id: ActiveValue::Unchanged(watermark_id),
            processing_status: ActiveValue::Set(status),
            pointer: ActiveValue::Set(pointer.to_owned()),
            upper_bound_timestamp: ActiveValue::Set(upper_bound_timestamp),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark_id))
        .exec(tx)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }
    #[instrument(level = "info", skip_all)]
    pub async fn process_realtime_cctxs(
        &self,
        job_id: Uuid,
        cctxs: Vec<CrossChainTx>,
        next_key: &str,
    ) -> anyhow::Result<Vec<CctxListItemProto>> {
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
            return Ok(Vec::new());
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

        let inserted = self.batch_insert_transactions(job_id, &cctxs, &tx).await?;

        tx.commit().await?;
        Ok(inserted)
    }
    #[instrument(level = "debug", skip_all)]
    pub async fn import_cctxs(
        &self,
        job_id: Uuid,
        cctxs: Vec<CrossChainTx>,
        next_key: &str,
        watermark_id: i32,
    ) -> anyhow::Result<Vec<CctxListItemProto>> {
        let tx = self.db.begin().await?;
        let upper_bound_timestamp:i64 = cctxs
            .last()
            .unwrap()
            .cctx_status
            .last_update_timestamp
            .parse::<i64>()
            .unwrap_or_default();
        let upper_bound_timestamp = DateTime::<Utc>::from_timestamp(upper_bound_timestamp, 0)
            .unwrap()
            .naive_utc();
        let imported = self.batch_insert_transactions(job_id, &cctxs, &tx).await?;
        //Watermark with id 1 is the main historic sync thread, all others are one-offs that might be manually added to the db
        let new_status = if watermark_id != 1 {
            ProcessingStatus::Done
        } else {
            ProcessingStatus::Unlocked
        };
        self.update_watermark(
            watermark_id,
            next_key,
            Some(upper_bound_timestamp),
            new_status,
            &tx,
        )
        .await?;
        tx.commit().await?;
        Ok(imported)
    }
    #[instrument(,level="trace",skip(self,job_id,watermark_id), fields(watermark_id = %watermark_id))]
    pub async fn unlock_watermark(&self, watermark_id: i32, job_id: Uuid) -> anyhow::Result<()> {
        let res = watermark::Entity::update(watermark::ActiveModel {
            id: ActiveValue::Unchanged(watermark_id),
            processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
            updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            updated_by: ActiveValue::Set(job_id.to_string()),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark_id))
        .exec(self.db.as_ref())
        .await?;

        tracing::debug!("unlocked watermark: {:?}", res);
        Ok(())
    }

    #[instrument(,level="trace",skip_all)]
    pub async fn lock_watermark(&self, watermark_id: i32, job_id: Uuid) -> anyhow::Result<()> {
        watermark::Entity::update(watermark::ActiveModel {
            id: ActiveValue::Unchanged(watermark_id),
            processing_status: ActiveValue::Set(ProcessingStatus::Locked),
            updated_by: ActiveValue::Set(job_id.to_string()),
            updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark_id))
        .exec(self.db.as_ref())
        .await?;
        Ok(())
    }

    #[instrument(,level="trace",skip(self),fields(watermark_id = %watermark_id,job_id = %job_id))]
    pub async fn mark_watermark_as_done(
        &self,
        watermark_id: i32,
        job_id: Uuid,
    ) -> anyhow::Result<()> {
        watermark::Entity::update(watermark::ActiveModel {
            id: ActiveValue::Unchanged(watermark_id),
            processing_status: ActiveValue::Set(ProcessingStatus::Done),
            ..Default::default()
        })
        .filter(watermark::Column::Id.eq(watermark_id))
        .exec(self.db.as_ref())
        .await?;
        Ok(())
    }

    #[instrument(,level="trace",skip_all)]
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

    #[instrument(,level="trace",skip(self),fields(batch_id = %batch_id))]
    pub async fn query_failed_cctxs(&self, batch_id: Uuid) -> anyhow::Result<Vec<CctxShort>> {
        let statement = format!(
            r#"
        WITH cctxs AS (
            SELECT cctx.id
            FROM cross_chain_tx cctx
            JOIN cctx_status cs ON cctx.id = cs.cross_chain_tx_id
            WHERE cctx.processing_status = 'Failed'::processing_status
            AND cctx.last_status_update_timestamp + INTERVAL '1 hour' * POWER(2, cctx.retries_number) < NOW()
            ORDER BY cctx.last_status_update_timestamp ASC,cs.created_timestamp DESC
            LIMIT 100
        )
        UPDATE cross_chain_tx cctx
        SET processing_status = 'Locked'::processing_status, last_status_update_timestamp = NOW(), retries_number = retries_number + 1
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

    #[instrument(,level="trace",skip_all)]
    pub async fn get_unlocked_watermarks(
        &self,
        kind: Kind,
    ) -> anyhow::Result<Vec<(i32, String, i32)>> {
        let query_statement = r#"
            WITH unlocked_watermarks AS (
                SELECT * FROM watermark WHERE kind::text = $1 AND processing_status::text = 'Unlocked'
                FOR UPDATE SKIP LOCKED
            )
            UPDATE watermark
            SET processing_status = 'Locked' ,updated_at = NOW()
            WHERE id IN (SELECT id FROM unlocked_watermarks)
            RETURNING id, pointer, retries_number
            "#;

        let query_statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            query_statement,
            vec![kind.to_string().into()],
        );

        let watermarks: Result<Vec<(i32, String, i32)>, sea_orm::DbErr> = self
            .db
            .query_all(query_statement)
            .await?
            .iter()
            .map(|r| {
                std::result::Result::Ok((
                    r.try_get_by_index(0)?,
                    r.try_get_by_index(1)?,
                    r.try_get_by_index(2)?,
                ))
            })
            .collect::<Result<Vec<(i32, String, i32)>, sea_orm::DbErr>>();

        watermarks.map_err(|e| anyhow::anyhow!(e))
    }

    #[instrument(,level="trace",skip(self), fields(batch_id = %batch_id))]
    pub async fn query_cctxs_for_status_update(
        &self,
        batch_size: u32,
        batch_id: Uuid,
        polling_interval: u64,
    ) -> anyhow::Result<Vec<CctxShort>> {
        // Optimized query that pre-calculates the backoff intervals to avoid complex expressions in WHERE clause
        let statement = format!(
            r#"
        WITH cctxs AS (
            SELECT cctx.id
            FROM cross_chain_tx cctx
            JOIN cctx_status cs ON cctx.id = cs.cross_chain_tx_id
            WHERE cctx.processing_status = 'Unlocked'::processing_status
            AND cctx.last_status_update_timestamp + 
                CASE 
                    WHEN cctx.retries_number = 0 THEN INTERVAL '0 milliseconds'
                    WHEN cctx.retries_number = 1 THEN INTERVAL '$1 milliseconds'
                    WHEN cctx.retries_number = 2 THEN INTERVAL '$1 milliseconds' * 3
                    WHEN cctx.retries_number = 3 THEN INTERVAL '$1 milliseconds' * 7
                    WHEN cctx.retries_number = 4 THEN INTERVAL '$1 milliseconds' * 15
                    WHEN cctx.retries_number = 5 THEN INTERVAL '$1 milliseconds' * 31
                    WHEN cctx.retries_number = 6 THEN INTERVAL '$1 milliseconds' * 63
                    WHEN cctx.retries_number = 7 THEN INTERVAL '$1 milliseconds' * 127
                    WHEN cctx.retries_number = 8 THEN INTERVAL '$1 milliseconds' * 255
                    WHEN cctx.retries_number = 9 THEN INTERVAL '$1 milliseconds' * 511
                    ELSE INTERVAL '$1 milliseconds' * 1023
                END < NOW()
            ORDER BY cctx.last_status_update_timestamp ASC, cs.created_timestamp DESC
            LIMIT $2
            FOR UPDATE SKIP LOCKED
        )
        UPDATE cross_chain_tx cctx
        SET processing_status = 'Locked'::processing_status, last_status_update_timestamp = NOW(), retries_number = retries_number + 1
        WHERE id IN (SELECT id FROM cctxs)
        RETURNING id, index, root_id, depth, retries_number
        "#
        );

        let statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            statement,
            vec![polling_interval.into(), batch_size.into()],
        );

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

    #[instrument(,level="trace",skip(self,tx),fields(cctx_id = %cctx_id))]
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

    #[instrument(,level="trace",skip(self,tx),fields(cctx_id = %cctx_id, job_id = %job_id))]
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

    #[instrument(,level="trace",skip_all)]
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
                    gas_used: ActiveValue::Set(outbound_params.gas_used.parse::<i64>()?),
                    effective_gas_price: ActiveValue::Set(outbound_params.effective_gas_price),
                    effective_gas_limit: ActiveValue::Set(
                        outbound_params.effective_gas_limit.parse::<i64>()?,
                    ),
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
                status: ActiveValue::Set(
                    CctxStatusStatus::try_from(fetched_cctx.cctx_status.status.clone())
                        .map_err(|e| anyhow::anyhow!(e))?,
                ),
                last_update_timestamp: ActiveValue::Set(
                    chrono::DateTime::from_timestamp(
                        fetched_cctx
                            .cctx_status
                            .last_update_timestamp
                            .parse::<i64>()
                            .unwrap_or(0),
                        0,
                    )
                    .ok_or(anyhow::anyhow!("Invalid timestamp"))?
                    .naive_utc(),
                ),
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
            sender_chain_id: ActiveValue::Set(cctx.inbound_params.sender_chain_id.parse::<i32>()?),
            tx_origin: ActiveValue::Set(cctx.inbound_params.tx_origin),
            coin_type: ActiveValue::Set(
                DBCoinType::try_from(cctx.inbound_params.coin_type)
                    .map_err(|e| anyhow::anyhow!(e))?,
            ),
            asset: ActiveValue::Set(Some(cctx.inbound_params.asset)),
            amount: ActiveValue::Set(cctx.inbound_params.amount),
            observed_hash: ActiveValue::Set(cctx.inbound_params.observed_hash),
            observed_external_height: ActiveValue::Set(
                cctx.inbound_params
                    .observed_external_height
                    .parse::<i64>()?,
            ),
            ballot_index: ActiveValue::Set(cctx.inbound_params.ballot_index),
            finalized_zeta_height: ActiveValue::Set(
                cctx.inbound_params.finalized_zeta_height.parse::<i64>()?,
            ),
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
                sea_orm::sea_query::OnConflict::columns([
                    InboundParamsEntity::Column::ObservedHash,
                    InboundParamsEntity::Column::BallotIndex,
                ])
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
                receiver_chain_id: ActiveValue::Set(outbound.receiver_chain_id.parse::<i32>()?),
                coin_type: ActiveValue::Set(
                    DBCoinType::try_from(outbound.coin_type).map_err(|e| anyhow::anyhow!(e))?,
                ),
                amount: ActiveValue::Set(outbound.amount),
                tss_nonce: ActiveValue::Set(outbound.tss_nonce),
                gas_limit: ActiveValue::Set(outbound.gas_limit),
                gas_price: ActiveValue::Set(Some(outbound.gas_price)),
                gas_priority_fee: ActiveValue::Set(Some(outbound.gas_priority_fee)),
                hash: ActiveValue::Set(if outbound.hash.is_empty() {
                    None
                } else {
                    Some(outbound.hash)
                }),
                ballot_index: ActiveValue::Set(Some(outbound.ballot_index)),
                observed_external_height: ActiveValue::Set(
                    outbound.observed_external_height.parse::<i64>()?,
                ),
                gas_used: ActiveValue::Set(outbound.gas_used.parse::<i64>()?),
                effective_gas_price: ActiveValue::Set(outbound.effective_gas_price),
                effective_gas_limit: ActiveValue::Set(outbound.effective_gas_limit.parse::<i64>()?),
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
                    sea_orm::sea_query::OnConflict::columns([
                        OutboundParamsEntity::Column::CrossChainTxId,
                        OutboundParamsEntity::Column::Receiver,
                        OutboundParamsEntity::Column::ReceiverChainId,
                    ])
                    .update_columns([
                        OutboundParamsEntity::Column::Hash,
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

    pub async fn get_sync_progress(&self) -> anyhow::Result<SyncProgress> {
        let historical_watermark = watermark::Entity::find()
            .filter(watermark::Column::Kind.eq(Kind::Historical))
            .one(self.db.as_ref())
            .await?
            .ok_or(anyhow::anyhow!("Historical watermark not found"))?;

        let historical_watermark_timestamp = historical_watermark
            .upper_bound_timestamp
            .expect("Historical watermark upper_bound_timestamp is not set");

        let realtime_gaps = watermark::Entity::find()
            .filter(watermark::Column::Kind.eq(Kind::Realtime))
            .filter(watermark::Column::ProcessingStatus.ne(ProcessingStatus::Done))
            .all(self.db.as_ref())
            .await?;

        let pending_cctxs_status_updates = CrossChainTxEntity::Entity::find()
            .filter(
                CrossChainTxEntity::Column::ProcessingStatus
                    .is_in(vec![ProcessingStatus::Locked, ProcessingStatus::Unlocked]),
            )
            .all(self.db.as_ref())
            .await?;

        let pending_cctxs = cctx_status::Entity::find()
            .filter(cctx_status::Column::Status.is_not_in(vec![
                CctxStatusStatus::OutboundMined,
                CctxStatusStatus::Aborted,
                CctxStatusStatus::Reverted,
            ]))
            .all(self.db.as_ref())
            .await?;

        Ok(SyncProgress {
            historical_watermark_timestamp,
            pending_status_updates_count: pending_cctxs_status_updates.len() as i64,
            pending_cctxs_count: pending_cctxs.len() as i64,
            realtime_gaps_count: realtime_gaps.len() as i64,
        })
    }

    #[instrument(level = "debug", skip_all, fields(index = %index))]
    pub async fn get_complete_cctx(
        &self,
        index: String,
    ) -> anyhow::Result<Option<CrossChainTxProto>> {
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
        ro.revert_gas_limit,--45
        COALESCE(erc20.symbol, gas.symbol) as token_symbol,--46
        COALESCE(erc20.zrc20_contract_address, gas.zrc20_contract_address) as zrc20_contract_address,--47
        COALESCE(erc20.icon_url, gas.icon_url) as icon_url,--48
        COALESCE(erc20.decimals, gas.decimals) as decimals,--49
        COALESCE(erc20.name, gas.name) as token_name--50

        FROM cross_chain_tx cctx
        LEFT JOIN cctx_status cs ON cctx.id = cs.cross_chain_tx_id
        LEFT JOIN inbound_params ip ON cctx.id = ip.cross_chain_tx_id
        LEFT JOIN revert_options ro ON cctx.id = ro.cross_chain_tx_id
        LEFT JOIN token erc20 ON ip.asset = erc20.asset
        LEFT JOIN token gas ON  gas.foreign_chain_id =ip.sender_chain_id and ip.coin_type = gas.coin_type and gas.coin_type  in ('Zeta', 'Gas')
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

        let id: i32 = cctx_row
            .try_get_by_index(0)
            .map_err(|e| anyhow::anyhow!("cctx_row id: {}", e))?;
        let creator: String = cctx_row
            .try_get_by_index(1)
            .map_err(|e| anyhow::anyhow!("cctx_row creator: {}", e))?;
        let index: String = cctx_row
            .try_get_by_index(2)
            .map_err(|e| anyhow::anyhow!("cctx_row index: {}", e))?;
        let zeta_fees: String = cctx_row
            .try_get_by_index(3)
            .map_err(|e| anyhow::anyhow!("cctx_row zeta_fees: {}", e))?;

        let relayed_message: String = cctx_row
            .try_get_by_index(6)
            .map_err(|e| anyhow::anyhow!("cctx_row relayed_message: {}", e))?;
        let protocol_contract_version: ProtocolContractVersion = ProtocolContractVersion::try_from(
            cctx_row.try_get_by_index::<String>(8)?,
        )
        .map_err(|_| sea_orm::DbErr::Custom("Invalid protocol_contract_version".to_string()))?;

        let last_update_timestamp: NaiveDateTime = cctx_row
            .try_get_by_index(18)
            .map_err(|e| anyhow::anyhow!("cctx_row last_update_timestamp: {}", e))?;

        // Build CctxStatus
        let status = StatusProto {
            status: CctxStatusProto::from_str_name(
                cctx_row.try_get_by_index::<String>(15)?.as_str(),
            )
            .ok_or(anyhow::anyhow!("Invalid status"))?
            .into(),
            status_message: cctx_row
                .try_get_by_index(16)
                .map_err(|e| anyhow::anyhow!("cctx_row status_message: {}", e))?,
            error_message: cctx_row
                .try_get_by_index(17)
                .map_err(|e| anyhow::anyhow!("cctx_row error_message: {}", e))?,
            last_update_timestamp: last_update_timestamp.and_utc().timestamp() as i64,
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
        let inbound = InboundParamsProto {
            sender: cctx_row
                .try_get_by_index(25)
                .map_err(|e| anyhow::anyhow!("cctx_row sender: {}", e))?,
            sender_chain_id: cctx_row
                .try_get_by_index::<i32>(26)
                .map_err(|e| anyhow::anyhow!("cctx_row sender_chain_id: {}", e))?
                .into(),
            tx_origin: cctx_row
                .try_get_by_index(27)
                .map_err(|e| anyhow::anyhow!("cctx_row tx_origin: {}", e))?,
            coin_type: CoinTypeProto::from_str_name(
                cctx_row.try_get_by_index::<String>(28)?.as_str(),
            )
            .ok_or(anyhow::anyhow!("Invalid coin_type"))?
            .into(),
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
                .try_get_by_index::<i64>(32)
                .map_err(|e| anyhow::anyhow!("cctx_row observed_external_height: {}", e))?,
            ballot_index: cctx_row
                .try_get_by_index(33)
                .map_err(|e| anyhow::anyhow!("cctx_row ballot_index: {}", e))?,
            finalized_zeta_height: cctx_row
                .try_get_by_index(34)
                .map_err(|e| anyhow::anyhow!("cctx_row finalized_zeta_height: {}", e))?,
            tx_finalization_status: TxFinalizationStatusProto::from_str_name(
                cctx_row.try_get_by_index::<String>(35)?.as_str(),
            )
            .ok_or(anyhow::anyhow!("Invalid tx_finalization_status"))?
            .into(),
            is_cross_chain_call: cctx_row
                .try_get_by_index(36)
                .map_err(|e| anyhow::anyhow!("cctx_row is_cross_chain_call: {}", e))?,
            status: InboundStatusProto::from_str_name(
                cctx_row.try_get_by_index::<String>(37)?.as_str(),
            )
            .ok_or(anyhow::anyhow!(
                "Invalid inbound_status:{}",
                cctx_row.try_get_by_index::<String>(37)?.as_str()
            ))?
            .into(),
            confirmation_mode: ConfirmationModeProto::from_str_name(
                cctx_row.try_get_by_index::<String>(38)?.as_str(),
            )
            .ok_or(anyhow::anyhow!("Invalid inbound_confirmation_mode"))?
            .into(),
        };

        let token_symbol: Option<String> = cctx_row
            .try_get_by_index(46)
            .map_err(|e| anyhow::anyhow!("cctx_row token_symbol: {}", e))?;
        let zrc20_contract_address: Option<String> = cctx_row
            .try_get_by_index(47)
            .map_err(|e| anyhow::anyhow!("cctx_row zrc20_contract_address: {}", e))?;
        let icon_url: Option<String> = cctx_row
            .try_get_by_index(48)
            .map_err(|e| anyhow::anyhow!("cctx_row icon_url: {}", e))?;
        let decimals: Option<i32> = cctx_row
            .try_get_by_index(49)
            .map_err(|e| anyhow::anyhow!("cctx_row decimals: {}", e))?;
        let token_name: Option<String> = cctx_row
            .try_get_by_index(50)
            .map_err(|e| anyhow::anyhow!("cctx_row token_name: {}", e))?;

        // Build RevertOptions
        let revert = RevertOptionsProto {
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
                .try_get_by_index::<String>(44)
                .map(|x| x.as_bytes().to_vec())
                .map_err(|e| anyhow::anyhow!("cctx_row revert_message: {}", e))
                .unwrap_or_default()
                .into(),
            revert_gas_limit: cctx_row
                .try_get_by_index(45)
                .map_err(|e| anyhow::anyhow!("cctx_row revert_gas_limit: {}", e))?,
        };

        // Now get outbound params (can be multiple, so we need a separate query)
        let outbounds_sql = r#"
        SELECT
            id, --0
            cross_chain_tx_id, --1
            receiver, --2
            receiver_chain_id, --3
            coin_type::text, --4
            amount, --5
            tss_nonce, --6
            gas_limit, --7
            gas_price, --8
            gas_priority_fee, --9
            hash, --10
            ballot_index, --11
            observed_external_height, --12
            gas_used, --13
            effective_gas_price, --14
            effective_gas_limit, --15
            tss_pubkey, --16
            tx_finalization_status::text, --17
            call_options_gas_limit, --18
            call_options_is_arbitrary_call, --19
            confirmation_mode::text --20
        FROM outbound_params
        WHERE cross_chain_tx_id = $1
        "#;

        let outbounds_statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            outbounds_sql,
            vec![sea_orm::Value::Int(Some(id))],
        );

        let outbounds_rows = self.db.query_all(outbounds_statement).await?;
        let mut outbounds = Vec::new();

        for outbound_row in outbounds_rows {
            outbounds.push(OutboundParamsProto {
                receiver: outbound_row
                    .try_get_by_index::<String>(2)
                    .map_err(|e| anyhow::anyhow!("outbound_row receiver: {}", e))?,
                receiver_chain_id: outbound_row
                    .try_get_by_index::<i32>(3)
                    .map_err(|e| anyhow::anyhow!("outbound_row receiver_chain_id: {}", e))?,
                coin_type: CoinTypeProto::from_str_name(
                    outbound_row
                        .try_get_by_index::<String>(4)
                        .map_err(|e| anyhow::anyhow!("outbound_row coin_type: {}", e))?
                        .as_str(),
                )
                .ok_or(anyhow::anyhow!("Invalid coin_type"))?
                .into(),
                amount: outbound_row
                    .try_get_by_index::<String>(5)
                    .map_err(|e| anyhow::anyhow!("outbound_row amount: {}", e))?,
                tss_nonce: outbound_row
                    .try_get_by_index::<String>(6)
                    .map_err(|e| anyhow::anyhow!("outbound_row tss_nonce: {}", e))?
                    .parse::<u64>()
                    .map_err(|e| anyhow::anyhow!("outbound_row tss_nonce: {}", e))?,
                gas_limit: outbound_row
                    .try_get_by_index::<String>(7)
                    .map_err(|e| anyhow::anyhow!("outbound_row gas_limit: {}", e))?
                    .parse::<u64>()
                    .map_err(|e| anyhow::anyhow!("outbound_row gas_limit: {}", e))?,
                gas_price: outbound_row
                    .try_get_by_index::<String>(8)
                    .map_err(|e| anyhow::anyhow!("outbound_row gas_price: {}", e))?,
                gas_priority_fee: outbound_row
                    .try_get_by_index::<String>(9)
                    .map_err(|e| anyhow::anyhow!("outbound_row gas_priority_fee: {}", e))?,
                hash: outbound_row
                    .try_get_by_index::<Option<String>>(10)
                    .map_err(|e| anyhow::anyhow!("outbound_row hash: {}", e))?
                    .map(|x| x.clone()),
                ballot_index: outbound_row
                    .try_get_by_index::<String>(11)
                    .map_err(|e| anyhow::anyhow!("outbound_row ballot_index: {}", e))?,
                observed_external_height: outbound_row
                    .try_get_by_index::<i64>(12)
                    .map_err(|e| anyhow::anyhow!("outbound_row observed_external_height: {}", e))?
                    as u64,
                gas_used: outbound_row
                    .try_get_by_index::<i64>(13)
                    .map_err(|e| anyhow::anyhow!("outbound_row gas_used: {}", e))?
                    as u64,
                effective_gas_price: outbound_row
                    .try_get_by_index::<String>(14)
                    .map_err(|e| anyhow::anyhow!("outbound_row effective_gas_price: {}", e))?,
                effective_gas_limit: outbound_row
                    .try_get_by_index::<i64>(15)
                    .map_err(|e| anyhow::anyhow!("outbound_row effective_gas_limit: {}", e))?,
                tss_pubkey: outbound_row
                    .try_get_by_index::<String>(16)
                    .map_err(|e| anyhow::anyhow!("outbound_row tss_pubkey: {}", e))?,
                tx_finalization_status: TxFinalizationStatusProto::from_str_name(
                    outbound_row
                        .try_get_by_index::<String>(17)
                        .unwrap()
                        .as_str(),
                )
                .ok_or(anyhow::anyhow!("Invalid tx_finalization_status"))?
                .into(),
                call_options: Some(CallOptionsProto {
                    gas_limit: outbound_row
                        .try_get_by_index::<Option<String>>(18)
                        .map_err(|e| anyhow::anyhow!("outbound_row gas_limit: {}", e))?
                        .map(|x| x.parse::<u64>().unwrap_or(0)),
                    is_arbitrary_call: outbound_row
                        .try_get_by_index::<Option<bool>>(19)
                        .map_err(|e| anyhow::anyhow!("outbound_row is_arbitrary_call: {}", e))?,
                }),
                confirmation_mode: ConfirmationModeProto::from_str_name(
                    outbound_row
                        .try_get_by_index::<String>(20)
                        .unwrap()
                        .as_str(),
                )
                .ok_or(anyhow::anyhow!("Invalid confirmation_mode"))?
                .into(),
            });
        }

        let related_sql = r#"
            select
            related_cctx.related_index, --0
            related_cctx.depth, --1
            ip.sender_chain_id as chain_id, --2
            cs.status::text as status, --3
            cs.created_timestamp, --4
            related_cctx.parent_index, --5
            COALESCE(erc20.name, gas.name) as name, --6
            COALESCE(erc20.symbol, gas.symbol) as symbol, --7
            COALESCE(erc20.decimals, gas.decimals) as decimals, --8
            COALESCE(erc20.zrc20_contract_address, gas.zrc20_contract_address) as zrc20_contract_address, --9
            COALESCE(erc20.icon_url, gas.icon_url) as icon_url, --10
            ip.amount, --11
            ip.coin_type::text, --12
            ip.asset, --13
            related_cctx.id --14
            from
                (
                    select
                        related.depth,
                        cctx.index as cctx_index,
                        related.index related_index,
                        related.id id,
                        parent.index as parent_index
                    from
                        cross_chain_tx cctx
                        join cross_chain_tx root on COALESCE(cctx.root_id, cctx.id) = root.id
                        join cross_chain_tx related on COALESCE(related.root_id, related.id) = root.id
                        -- and related.id != cctx.id
                        left join cross_chain_tx parent on related.parent_id = parent.id
                ) as related_cctx
                join inbound_params ip on related_cctx.id = ip.cross_chain_tx_id
                join cctx_status cs on related_cctx.id = cs.cross_chain_tx_id
                left join token erc20 on ip.asset = erc20.asset and erc20.coin_type::text = 'Erc20'
                left join token gas on ip.sender_chain_id = gas.foreign_chain_id and gas.coin_type::text in ( 'Gas', 'Zeta' ) and ip.coin_type = gas.coin_type
            where
                cctx_index = $1
            order by related_cctx.depth asc;
"#;

        let related_statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            related_sql,
            vec![sea_orm::Value::String(Some(Box::new(index.clone())))],
        );

        let related_rows = self
            .db
            .query_all(related_statement)
            .await
            .map_err(|e| anyhow::anyhow!("related_rows: {}", e))?;
        let mut related = Vec::new();

        for related_row in related_rows {
            let related_id: i32 = related_row
                .try_get_by_index(14)
                .map_err(|e| anyhow::anyhow!("related_row related_id: {}", e))?;
            let outbound_params = OutboundParamsEntity::Entity::find()
                .filter(OutboundParamsEntity::Column::CrossChainTxId.eq(related_id))
                .all(self.db.as_ref())
                .await?
                .into_iter()
                .map(|x| RelatedOutboundParamsProto {
                    amount: x.amount,
                    chain_id: x.receiver_chain_id,
                    coin_type: x.coin_type.into(),
                    gas_used: x.gas_used,
                })
                .collect();

            let status = CctxStatusProto::from_str_name(
                related_row
                    .try_get_by_index::<String>(3)
                    .map_err(|e| anyhow::anyhow!("related_row status: {}", e))?
                    .as_str(),
            )
            .ok_or(anyhow::anyhow!("Invalid status"))?;

            related.push(RelatedCctxProto {
                index: related_row
                    .try_get_by_index::<String>(0)
                    .map_err(|e| anyhow::anyhow!("related_row index: {}", e))?,
                depth: related_row
                    .try_get_by_index::<i32>(1)
                    .map_err(|e| anyhow::anyhow!("related_row depth: {}", e))?,
                source_chain_id: related_row
                    .try_get_by_index::<i32>(2)
                    .map_err(|e| anyhow::anyhow!("related_row source_chain_id: {}", e))?,
                status: status.into(),
                status_reduced: reduce_status(status).into(),
                created_timestamp: related_row
                    .try_get_by_index::<i64>(4)
                    .map_err(|e| anyhow::anyhow!("related_row created_timestamp: {}", e))?,
                parent_index: related_row
                    .try_get_by_index::<Option<String>>(5)
                    .map_err(|e| anyhow::anyhow!("related_row parent_index: {}", e))?,
                token_name: related_row
                    .try_get_by_index::<Option<String>>(6)
                    .map_err(|e| anyhow::anyhow!("related_row token_name: {}", e))?,
                token_symbol: related_row
                    .try_get_by_index::<Option<String>>(7)
                    .map_err(|e| anyhow::anyhow!("related_row token_symbol: {}", e))?,
                token_decimals: related_row
                    .try_get_by_index::<Option<i32>>(8)
                    .map_err(|e| anyhow::anyhow!("related_row token_decimals: {}", e))?,
                token_zrc20_contract_address: related_row
                    .try_get_by_index::<Option<String>>(9)
                    .map_err(|e| {
                        anyhow::anyhow!("related_row token_zrc20_contract_address: {}", e)
                    })?,
                token_icon_url: related_row
                    .try_get_by_index::<Option<String>>(10)
                    .map_err(|e| anyhow::anyhow!("related_row token_icon_url: {}", e))?,
                inbound_amount: related_row
                    .try_get_by_index::<String>(11)
                    .map_err(|e| anyhow::anyhow!("related_row inbound_amount: {}", e))?,
                inbound_coin_type: CoinTypeProto::from_str_name(
                    related_row
                        .try_get_by_index::<String>(12)
                        .map_err(|e| anyhow::anyhow!("related_row inbound_coin_type: {}", e))?
                        .as_str(),
                )
                .ok_or(anyhow::anyhow!("Invalid coin_type"))?
                .into(),
                inbound_asset: related_row
                    .try_get_by_index::<Option<String>>(13)
                    .map_err(|e| anyhow::anyhow!("related_row inbound_asset: {}", e))?,
                outbound_params,
            });
        }

        let status_reduced = reduce_status(
            CctxStatusProto::try_from(status.status.clone())
                .unwrap()
                .into(),
        );

        Ok(Some(CrossChainTxProto {
            creator: creator,
            index: index,
            zeta_fees: zeta_fees,
            relayed_message: relayed_message,
            cctx_status: Some(status),
            cctx_status_reduced: status_reduced.into(),
            inbound_params: Some(inbound),
            outbound_params: outbounds,
            revert_options: Some(revert),
            protocol_contract_version: protocol_contract_version.into(),
            related_cctxs: related,
            token_symbol: token_symbol,
            token_name: token_name,
            zrc20_contract_address: zrc20_contract_address,
            icon_url: icon_url,
            decimals: decimals,
        }))
    }

    #[instrument(level = "info", skip_all)]
    pub async fn list_cctxs(
        &self,
        limit: i64,
        page_key: Option<i64>,
        filters: Filters,
        direction: Direction,
    ) -> anyhow::Result<ListCctxsResponse> {
        let mut sql = include_str!("list_cctx_query.sql").to_string();

        let mut params: Vec<sea_orm::Value> = Vec::new();
        let mut param_count = 0;

        if !filters.status_reduced.is_empty() {
            param_count += 1;
            sql.push_str(&format!(" AND status_reduced = ANY(${})", param_count));
            params.push(sea_orm::Value::Array(
                sea_orm::sea_query::ArrayType::String,
                Some(Box::new(
                    filters
                        .status_reduced
                        .into_iter()
                        .map(|s| sea_orm::Value::String(Some(Box::new(s))))
                        .collect::<Vec<_>>(),
                )),
            ));
        }

        if !filters.sender_address.is_empty() {
            param_count += 1;
            sql.push_str(&format!(" AND sender_address = ANY(${})", param_count));
            params.push(sea_orm::Value::Array(
                sea_orm::sea_query::ArrayType::String,
                Some(Box::new(
                    filters
                        .sender_address
                        .into_iter()
                        .map(|s| sea_orm::Value::String(Some(Box::new(s))))
                        .collect::<Vec<_>>(),
                )),
            ));
        }

        if !filters.receiver_address.is_empty() {
            param_count += 1;
            sql.push_str(&format!(" AND receiver_address = ANY(${})", param_count));
            params.push(sea_orm::Value::Array(
                sea_orm::sea_query::ArrayType::String,
                Some(Box::new(
                    filters
                        .receiver_address
                        .into_iter()
                        .map(|s| sea_orm::Value::String(Some(Box::new(s))))
                        .collect::<Vec<_>>(),
                )),
            ));
        }

        if !filters.asset.is_empty() {
            param_count += 1;
            sql.push_str(&format!(" AND asset = ANY(${})", param_count));
            params.push(sea_orm::Value::Array(
                sea_orm::sea_query::ArrayType::String,
                Some(Box::new(
                    filters
                        .asset
                        .into_iter()
                        .map(|s| sea_orm::Value::String(Some(Box::new(s))))
                        .collect::<Vec<_>>(),
                )),
            ));
        }

        if !filters.coin_type.is_empty() {
            param_count += 1;
            sql.push_str(&format!(" AND coin_type::text = ANY(${})", param_count));
            params.push(sea_orm::Value::Array(
                sea_orm::sea_query::ArrayType::String,
                Some(Box::new(
                    filters
                        .coin_type
                        .into_iter()
                        .map(|s| sea_orm::Value::String(Some(Box::new(s))))
                        .collect::<Vec<_>>(),
                )),
            ));
        }

        if !filters.source_chain_id.is_empty() {
            param_count += 1;
            sql.push_str(&format!(" AND sender_chain_id = ANY(${})", param_count));
            params.push(sea_orm::Value::Array(
                sea_orm::sea_query::ArrayType::Int,
                Some(Box::new(
                    filters
                        .source_chain_id
                        .into_iter()
                        .map(|s| sea_orm::Value::Int(Some(s)))
                        .collect::<Vec<_>>(),
                )),
            ));
        }

        if !filters.target_chain_id.is_empty() {
            param_count += 1;
            sql.push_str(&format!(" AND receiver_chain_id = ANY(${})", param_count));
            params.push(sea_orm::Value::Array(
                sea_orm::sea_query::ArrayType::Int,
                Some(Box::new(
                    filters
                        .target_chain_id
                        .into_iter()
                        .map(|s| sea_orm::Value::Int(Some(s)))
                        .collect::<Vec<_>>(),
                )),
            ));
        }

        if let Some(start_timestamp) = filters.start_timestamp {
            param_count += 1;
            sql.push_str(&format!(" AND created_timestamp >= ${}", param_count));
            params.push(sea_orm::Value::BigInt(Some(start_timestamp)));
        }

        if let Some(end_timestamp) = filters.end_timestamp {
            param_count += 1;
            sql.push_str(&format!(" AND created_timestamp <= ${}", param_count));
            params.push(sea_orm::Value::BigInt(Some(end_timestamp)));
        }

        if let Some(page_key) = page_key {
            param_count += 1;
            if direction == Direction::Asc {
                sql.push_str(&format!(" AND id > ${}", param_count));
            } else {
                sql.push_str(&format!(" AND id < ${}", param_count));
            }
            params.push(sea_orm::Value::BigInt(Some(page_key)));
        }

        if !filters.token_symbol.is_empty() {
            param_count += 1;
            sql.push_str(&format!(" AND token_symbol = ANY(${})", param_count));
            params.push(sea_orm::Value::Array(
                sea_orm::sea_query::ArrayType::String,
                Some(Box::new(
                    filters
                        .token_symbol
                        .into_iter()
                        .map(|s| sea_orm::Value::String(Some(Box::new(s))))
                        .collect::<Vec<_>>(),
                )),
            ));
        }

        // No need to group since we're only getting the last outbound_params row per CCTX
        sql.push_str(" ");

        // Add ordering and pagination
        param_count += 1;
        sql.push_str(&format!(
            " ORDER BY id {} LIMIT ${}",
            direction.as_str_name(),
            param_count
        ));
        params.push(sea_orm::Value::BigInt(Some(limit + 1)));

        let statement = Statement::from_sql_and_values(DbBackend::Postgres, sql.clone(), params);
        tracing::info!("statement: {}", statement.to_string());
        let rows = self
            .db
            .query_all(statement)
            .await
            .map_err(|e| anyhow::anyhow!("statement returned error: {}", e))?;
        let next_page_items_len = rows.len() - std::cmp::min(limit as usize, rows.len());
        let truncated = rows.iter().take(limit as usize);
        let ids = truncated.clone().map(|row| {
            row.try_get_by_index::<i64>(15)
                .map_err(|e| anyhow::anyhow!("row.try_get_by_index(15): {}", e))
                .unwrap_or_default()
        });
        let next_page_key: i64 = {
            if direction == Direction::Asc {
                ids.max().unwrap_or_default() as i64
            } else {
                ids.min().unwrap_or_default() as i64
            }
        };
        let mut items = Vec::new();
        for row in truncated {
            // Read the status enum directly from the database
            let index = row
                .try_get_by_index(0)
                .map_err(|e| anyhow::anyhow!("index: {}", e))?;
            let status = row
                .try_get_by_index::<String>(1)
                .map_err(|e| anyhow::anyhow!("status: {}", e))?;
            let status = CctxStatusProto::from_str_name(&status).unwrap();

            let last_update_timestamp = row
                .try_get_by_index::<NaiveDateTime>(2)
                .map_err(|e| anyhow::anyhow!("last_update_timestamp: {}", e))?;
            let amount = row
                .try_get_by_index(3)
                .map_err(|e| anyhow::anyhow!("amount: {}", e))?;
            let source_chain_id = row
                .try_get_by_index(4)
                .map_err(|e| anyhow::anyhow!("source_chain_id: {}", e))?;
            let target_chain_id = row
                .try_get_by_index(5)
                .map_err(|e| anyhow::anyhow!("target_chain_id: {}", e))?;
            let created_timestamp = row
                .try_get_by_index::<i64>(6)
                .map_err(|e| anyhow::anyhow!("created_timestamp: {}", e))?;
            let sender_address = row
                .try_get_by_index(7)
                .map_err(|e| anyhow::anyhow!("sender_address: {}", e))?;
            let asset = row
                .try_get_by_index(8)
                .map_err(|e| anyhow::anyhow!("asset: {}", e))?;
            let receiver_address = row
                .try_get_by_index(9)
                .map_err(|e| anyhow::anyhow!("receiver_address: {}", e))?;
            let coin_type = row
                .try_get_by_index::<String>(10)
                .map_err(|e| anyhow::anyhow!("coin_type: {}", e))?;
            let status_reduced = row
                .try_get_by_index::<String>(11)
                .map_err(|e| anyhow::anyhow!("status_reduced: {}", e))?;
            let status_reduced = CctxStatusReducedProto::from_str_name(&status_reduced).unwrap();
            let token_symbol: Option<String> = row
                .try_get_by_index(12)
                .map_err(|e| anyhow::anyhow!("token_symbol: {}", e))?;
            let coin_type = CoinTypeProto::from_str_name(&coin_type).unwrap();
            let zrc20_contract_address: Option<String> = row
                .try_get_by_index(13)
                .map_err(|e| anyhow::anyhow!("zrc20_contract_address: {}", e))?;
            let decimals: Option<i32> = row
                .try_get_by_index(14)
                .map_err(|e| anyhow::anyhow!("decimals: {}", e))?;

            items.push(CctxListItemProto {
                index,
                status: status.into(),
                status_reduced: status_reduced.into(),
                amount,
                created_timestamp,
                last_update_timestamp: last_update_timestamp.and_utc().timestamp(),
                source_chain_id,
                target_chain_id,
                sender_address,
                receiver_address,
                asset,
                coin_type: coin_type.into(),
                token_symbol,
                zrc20_contract_address,
                decimals: decimals.map(|x| x as i64),
            });
        }

        Ok(ListCctxsResponse {
            items,
            next_page_params: Some(Pagination {
                page_key: next_page_key,
                limit: next_page_items_len as u32,
                direction: direction.into(),
            }),
        })
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn sync_tokens(&self, job_id: Uuid, tokens: Vec<Token>) -> anyhow::Result<()> {
        if tokens.is_empty() {
            return Ok(());
        }

        let mut token_models: Vec<TokenEntity::ActiveModel> = Vec::new();
        for token in tokens {
            let active_model = token.try_into()?;
            token_models.push(active_model);
        }

        let insert_result = TokenEntity::Entity::insert_many(token_models)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(TokenEntity::Column::Zrc20ContractAddress)
                    .update_columns([
                        TokenEntity::Column::Asset,
                        TokenEntity::Column::ForeignChainId,
                        TokenEntity::Column::Decimals,
                        TokenEntity::Column::Name,
                        TokenEntity::Column::Symbol,
                        TokenEntity::Column::CoinType,
                        TokenEntity::Column::GasLimit,
                        TokenEntity::Column::Paused,
                        TokenEntity::Column::LiquidityCap,
                        TokenEntity::Column::UpdatedAt,
                        TokenEntity::Column::IconUrl,
                    ])
                    .to_owned(),
            )
            .exec_with_returning_many(self.db.as_ref())
            .await?;

        tracing::debug!("Synced {} tokens for job {}", insert_result.len(), job_id);
        Ok(())
    }

    pub async fn get_token_by_asset(&self, asset: &str) -> anyhow::Result<Option<TokenProto>> {
        let token = TokenEntity::Entity::find()
            .filter(TokenEntity::Column::Asset.eq(asset))
            .one(self.db.as_ref())
            .await?
            .map(|t| t.into());

        Ok(token)
    }

    #[instrument(skip_all, level = "debug")]
    pub async fn batch_insert_transactions(
        &self,
        job_id: Uuid,
        cctxs: &Vec<CrossChainTx>,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<Vec<CctxListItemProto>> {
        // Early return for empty input.
        if cctxs.is_empty() {
            return Ok(Vec::new());
        }

        // 1. Prepare and insert parent CrossChainTx records.
        let cctx_models = Self::prepare_parent_models(job_id, cctxs)?;
        let inserted_cctxs = self.insert_parent_models(cctx_models).await?;

        // 2. Map fetched cctxs by index for quick access when preparing child records.
        let index_to_cctx = cctxs
            .iter()
            .map(|cctx| (cctx.index.as_str(), cctx))
            .collect::<std::collections::HashMap<_, _>>();

        // 3. Prepare child entity models and construct the protobuf response.
        let (status_models, inbound_models, outbound_models, revert_models, cctx_list_items) =
            Self::prepare_child_models(&inserted_cctxs, &index_to_cctx)?;

        // 4. Persist child entities inside the provided transaction.
        self.insert_cctx_status_models(status_models, tx).await?;
        self.insert_inbound_params_models(inbound_models, tx)
            .await?;
        self.insert_outbound_params_models(outbound_models, tx)
            .await?;
        self.insert_revert_options_models(revert_models, tx).await?;

        Ok(cctx_list_items)
    }

    fn prepare_parent_models(
        job_id: Uuid,
        cctxs: &Vec<CrossChainTx>,
    ) -> anyhow::Result<Vec<CrossChainTxEntity::ActiveModel>> {
        let mut models = Vec::new();
        for cctx in cctxs {
            let model = CrossChainTxEntity::ActiveModel {
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
                root_id: ActiveValue::Set(None),
                parent_id: ActiveValue::Set(None),
                depth: ActiveValue::Set(0),
                updated_by: ActiveValue::Set(job_id.to_string()),
            };
            models.push(model);
        }
        Ok(models)
    }

    async fn insert_parent_models(
        &self,
        cctx_models: Vec<CrossChainTxEntity::ActiveModel>,
    ) -> anyhow::Result<Vec<CrossChainTxEntity::Model>> {
        if cctx_models.is_empty() {
            return Ok(Vec::new());
        }
        let indices = cctx_models
            .iter()
            .map(|m| m.index.as_ref().clone())
            .collect::<Vec<String>>();
        let inserted = CrossChainTxEntity::Entity::insert_many(cctx_models)
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

        tracing::info!("inserting cctxs: {}", indices.join(","));
        Ok(inserted)
    }

    fn prepare_child_models(
        inserted_cctxs: &Vec<CrossChainTxEntity::Model>,
        index_to_cctx: &std::collections::HashMap<&str, &CrossChainTx>,
    ) -> anyhow::Result<(
        Vec<cctx_status::ActiveModel>,
        Vec<InboundParamsEntity::ActiveModel>,
        Vec<OutboundParamsEntity::ActiveModel>,
        Vec<RevertOptionsEntity::ActiveModel>,
        Vec<CctxListItemProto>,
    )> {
        let mut status_models = Vec::new();
        let mut inbound_models = Vec::new();
        let mut outbound_models = Vec::new();
        let mut revert_models = Vec::new();
        let mut cctx_list_items = Vec::new();

        for cctx in inserted_cctxs {
            if let Some(fetched_cctx) = index_to_cctx.get(cctx.index.as_str()) {
                // Prepare status model
                let last_update_timestamp = fetched_cctx
                    .cctx_status
                    .last_update_timestamp
                    .parse::<i64>()
                    .unwrap_or(0);
                let last_update_timestamp =
                    chrono::DateTime::<Utc>::from_timestamp(last_update_timestamp, 0)
                        .ok_or(anyhow::anyhow!("Invalid timestamp"))?
                        .naive_utc();

                status_models.push(cctx_status::ActiveModel {
                    id: ActiveValue::NotSet,
                    cross_chain_tx_id: ActiveValue::Set(cctx.id),
                    status: ActiveValue::Set(
                        CctxStatusStatus::try_from(fetched_cctx.cctx_status.status.clone())
                            .map_err(|e| anyhow::anyhow!(e))?,
                    ),
                    status_message: ActiveValue::Set(Some(sanitize_string(
                        fetched_cctx.cctx_status.status_message.clone(),
                    ))),
                    error_message: ActiveValue::Set(Some(sanitize_string(
                        fetched_cctx.cctx_status.error_message.clone(),
                    ))),
                    last_update_timestamp: ActiveValue::Set(last_update_timestamp),
                    is_abort_refunded: ActiveValue::Set(fetched_cctx.cctx_status.is_abort_refunded),
                    created_timestamp: ActiveValue::Set(
                        fetched_cctx.cctx_status.created_timestamp.parse::<i64>()?,
                    ),
                    error_message_revert: ActiveValue::Set(Some(sanitize_string(
                        fetched_cctx.cctx_status.error_message_revert.clone(),
                    ))),
                    error_message_abort: ActiveValue::Set(Some(sanitize_string(
                        fetched_cctx.cctx_status.error_message_abort.clone(),
                    ))),
                });

                // Prepare inbound params model
                inbound_models.push(InboundParamsEntity::ActiveModel {
                    id: ActiveValue::NotSet,
                    cross_chain_tx_id: ActiveValue::Set(cctx.id),
                    sender: ActiveValue::Set(fetched_cctx.inbound_params.sender.clone()),
                    sender_chain_id: ActiveValue::Set(
                        fetched_cctx.inbound_params.sender_chain_id.parse::<i32>()?,
                    ),
                    tx_origin: ActiveValue::Set(fetched_cctx.inbound_params.tx_origin.clone()),
                    coin_type: ActiveValue::Set(
                        DBCoinType::try_from(fetched_cctx.inbound_params.coin_type.clone())
                            .map_err(|e| anyhow::anyhow!(e))?,
                    ),
                    asset: ActiveValue::Set(Some(fetched_cctx.inbound_params.asset.clone())),
                    amount: ActiveValue::Set(fetched_cctx.inbound_params.amount.clone()),
                    observed_hash: ActiveValue::Set(
                        fetched_cctx.inbound_params.observed_hash.clone(),
                    ),
                    observed_external_height: ActiveValue::Set(
                        fetched_cctx
                            .inbound_params
                            .observed_external_height
                            .parse::<i64>()?,
                    ),
                    ballot_index: ActiveValue::Set(
                        fetched_cctx.inbound_params.ballot_index.clone(),
                    ),
                    finalized_zeta_height: ActiveValue::Set(
                        fetched_cctx
                            .inbound_params
                            .finalized_zeta_height
                            .parse::<i64>()?,
                    ),
                    tx_finalization_status: ActiveValue::Set(
                        TxFinalizationStatus::try_from(
                            fetched_cctx.inbound_params.tx_finalization_status.clone(),
                        )
                        .map_err(|e| anyhow::anyhow!(e))?,
                    ),
                    is_cross_chain_call: ActiveValue::Set(
                        fetched_cctx.inbound_params.is_cross_chain_call,
                    ),
                    status: ActiveValue::Set(
                        InboundStatus::try_from(fetched_cctx.inbound_params.status.clone())
                            .map_err(|e| anyhow::anyhow!(e))?,
                    ),
                    confirmation_mode: ActiveValue::Set(
                        ConfirmationMode::try_from(
                            fetched_cctx.inbound_params.confirmation_mode.clone(),
                        )
                        .map_err(|e| anyhow::anyhow!(e))?,
                    ),
                });

                // Prepare outbound models
                for outbound in &fetched_cctx.outbound_params {
                    outbound_models.push(OutboundParamsEntity::ActiveModel {
                        id: ActiveValue::NotSet,
                        cross_chain_tx_id: ActiveValue::Set(cctx.id),
                        receiver: ActiveValue::Set(outbound.receiver.clone()),
                        receiver_chain_id: ActiveValue::Set(
                            outbound.receiver_chain_id.parse::<i32>()?,
                        ),
                        coin_type: ActiveValue::Set(
                            DBCoinType::try_from(outbound.coin_type.clone())
                                .map_err(|e| anyhow::anyhow!(e))?,
                        ),
                        amount: ActiveValue::Set(outbound.amount.clone()),
                        tss_nonce: ActiveValue::Set(outbound.tss_nonce.clone()),
                        gas_limit: ActiveValue::Set(outbound.gas_limit.clone()),
                        gas_price: ActiveValue::Set(Some(outbound.gas_price.clone())),
                        gas_priority_fee: ActiveValue::Set(Some(outbound.gas_priority_fee.clone())),
                        hash: ActiveValue::Set(if outbound.hash.is_empty() {
                            None
                        } else {
                            Some(outbound.hash.clone())
                        }),
                        ballot_index: ActiveValue::Set(Some(outbound.ballot_index.clone())),
                        observed_external_height: ActiveValue::Set(
                            outbound.observed_external_height.parse::<i64>()?,
                        ),
                        gas_used: ActiveValue::Set(outbound.gas_used.parse::<i64>()?),
                        effective_gas_price: ActiveValue::Set(outbound.effective_gas_price.clone()),
                        effective_gas_limit: ActiveValue::Set(
                            outbound.effective_gas_limit.parse::<i64>()?,
                        ),
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
                    });
                }

                // Prepare revert options model
                revert_models.push(RevertOptionsEntity::ActiveModel {
                    id: ActiveValue::NotSet,
                    cross_chain_tx_id: ActiveValue::Set(cctx.id),
                    revert_address: ActiveValue::Set(Some(
                        fetched_cctx.revert_options.revert_address.clone(),
                    )),
                    call_on_revert: ActiveValue::Set(fetched_cctx.revert_options.call_on_revert),
                    abort_address: ActiveValue::Set(Some(
                        fetched_cctx.revert_options.abort_address.clone(),
                    )),
                    revert_message: ActiveValue::Set(
                        fetched_cctx
                            .revert_options
                            .revert_message
                            .clone()
                            .map(|s| sanitize_string(s)),
                    ),
                    revert_gas_limit: ActiveValue::Set(
                        fetched_cctx.revert_options.revert_gas_limit.clone(),
                    ),
                });

                // Prepare proto list item
                cctx_list_items.push(CctxListItemProto {
                    index: cctx.index.clone(),
                    status: CctxStatusProto::from_str_name(&fetched_cctx.cctx_status.status)
                        .unwrap()
                        .into(),
                    status_reduced: 0,
                    amount: fetched_cctx.inbound_params.amount.clone(),
                    last_update_timestamp: fetched_cctx
                        .cctx_status
                        .last_update_timestamp
                        .parse::<i64>()
                        .unwrap_or(0),
                    created_timestamp: fetched_cctx
                        .cctx_status
                        .created_timestamp
                        .parse::<i64>()
                        .unwrap_or(0),
                    source_chain_id: fetched_cctx.inbound_params.sender_chain_id.parse::<i32>()?,
                    target_chain_id: fetched_cctx.outbound_params[0]
                        .receiver_chain_id
                        .parse::<i32>()?,
                    sender_address: fetched_cctx.inbound_params.sender.clone(),
                    receiver_address: fetched_cctx.outbound_params[0].receiver.clone(),
                    asset: fetched_cctx.inbound_params.asset.clone(),
                    coin_type: CoinTypeProto::from_str_name(
                        &fetched_cctx.inbound_params.coin_type.to_string(),
                    )
                    .unwrap()
                    .into(),
                    token_symbol: None,
                    zrc20_contract_address: None,
                    decimals: None,
                });
            }
        }

        Ok((
            status_models,
            inbound_models,
            outbound_models,
            revert_models,
            cctx_list_items,
        ))
    }

    async fn insert_cctx_status_models(
        &self,
        status_models: Vec<cctx_status::ActiveModel>,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        if status_models.is_empty() {
            return Ok(());
        }
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
        Ok(())
    }

    async fn insert_inbound_params_models(
        &self,
        inbound_models: Vec<InboundParamsEntity::ActiveModel>,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        if inbound_models.is_empty() {
            return Ok(());
        }
        InboundParamsEntity::Entity::insert_many(inbound_models)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([
                    InboundParamsEntity::Column::BallotIndex,
                    InboundParamsEntity::Column::ObservedHash,
                ])
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
        Ok(())
    }

    async fn insert_outbound_params_models(
        &self,
        outbound_models: Vec<OutboundParamsEntity::ActiveModel>,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        if outbound_models.is_empty() {
            return Ok(());
        }
        OutboundParamsEntity::Entity::insert_many(outbound_models)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([
                    OutboundParamsEntity::Column::CrossChainTxId,
                    OutboundParamsEntity::Column::Receiver,
                    OutboundParamsEntity::Column::ReceiverChainId,
                ])
                .update_columns([
                    OutboundParamsEntity::Column::Hash,
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
        Ok(())
    }

    async fn insert_revert_options_models(
        &self,
        revert_models: Vec<RevertOptionsEntity::ActiveModel>,
        tx: &DatabaseTransaction,
    ) -> anyhow::Result<()> {
        if revert_models.is_empty() {
            return Ok(());
        }
        RevertOptionsEntity::Entity::insert_many(revert_models)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(RevertOptionsEntity::Column::CrossChainTxId)
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
        Ok(())
    }

    // -------------------------------------------------------------------------

    pub async fn list_tokens(&self) -> anyhow::Result<Vec<TokenProto>> {
        let tokens = TokenEntity::Entity::find()
            .order_by_asc(TokenEntity::Column::Id)
            .all(self.db.as_ref())
            .await?;
        Ok(tokens.into_iter().map(|token| token.into()).collect())
    }
}
