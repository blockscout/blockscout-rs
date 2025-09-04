use std::{collections::HashMap, sync::Arc};

use crate::models::{self, CctxShort, CrossChainTx, Filters, SyncProgress, Token};
use anyhow::Ok;
use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{
    ActiveValue, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend,
    EntityTrait, QueryFilter, QueryOrder, Statement, TransactionTrait,
};
use tracing::{instrument, Instrument};
use uuid::Uuid;
use zetachain_cctx_entity::{
    cctx_status, cross_chain_tx as CrossChainTxEntity, inbound_params as InboundParamsEntity,
    outbound_params as OutboundParamsEntity, revert_options as RevertOptionsEntity,
    sea_orm_active_enums::{
        CctxStatusStatus, CoinType as DBCoinType, ConfirmationMode, InboundStatus, Kind,
        ProcessingStatus, ProtocolContractVersion, TxFinalizationStatus,
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
    zetachain_id: i32,
}

struct PreparedParentModels {
    models: Vec<CrossChainTxEntity::ActiveModel>,
    token_map: HashMap<String, Option<TokenEntity::Model>>,
}

struct ChildEntities {
    inbound_params: Vec<InboundParamsEntity::ActiveModel>,
    outbound_params: Vec<OutboundParamsEntity::ActiveModel>,
    revert_options: Vec<RevertOptionsEntity::ActiveModel>,
    status: Vec<cctx_status::ActiveModel>,
    cctx_list_items: Vec<CctxListItemProto>,
}

pub struct UnlockedWatermark {
    pub watermark_id: i32,
    pub retries_number: i32,
    pub upper_bound_timestamp: Option<NaiveDateTime>,
    pub pointer: String,
}

#[derive(Copy, Clone)]
pub struct Relation {
    pub parent_id: i32,
    pub depth: i32,
    pub root_id: i32,
}

pub type NeedUpdate = HashMap<i32, models::CrossChainTx>;

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
    pub fn new(db: Arc<DatabaseConnection>, zetachain_id: i32) -> Self {
        Self { db, zetachain_id }
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
    ) -> anyhow::Result<(Vec<models::CrossChainTx>, NeedUpdate)> {
        if fetched_cctxs.is_empty() {
            return Ok((Vec::new(), HashMap::new()));
        }
        let mut need_to_import = Vec::new();
        let mut need_to_update_ids = HashMap::new();

        for fetched_cctx in fetched_cctxs {
            let cctx = CrossChainTxEntity::Entity::find()
                .filter(CrossChainTxEntity::Column::Index.eq(fetched_cctx.index.clone()))
                .one(tx)
                .await?;
            if cctx.is_some() {
                let cctx = cctx.unwrap();
                if cctx.parent_id != Some(parent_id) || cctx.root_id != Some(root_id) {
                    need_to_update_ids.insert(cctx.id, fetched_cctx);
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

    #[instrument(level = "trace", skip_all)]
    pub async fn traverse_and_update_tree_relationships(
        &self,
        children_cctxs: Vec<CrossChainTx>,
        cctx: &CctxShort,
        job_id: Uuid,
    ) -> anyhow::Result<Vec<CctxListItemProto>> {
        let cctx_status = self.get_cctx_status(cctx.id).await?;
        let root_id = cctx.root_id.unwrap_or(cctx.id);
        let tx = self.db.begin().await?;
        let (need_to_import, need_to_update) = self
            .find_outdated_children_by_index(children_cctxs, cctx.id, root_id, &tx)
            .await?;

        let inserted = self
            .batch_insert_transactions(
                job_id,
                &need_to_import,
                &tx,
                Some(Relation {
                    parent_id: cctx.id,
                    depth: cctx.depth + 1,
                    root_id,
                }),
            )
            .await?;

        let need_to_update_ids = need_to_update.keys().cloned().collect::<Vec<_>>();
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
        if cctx_status.status == CctxStatusStatus::OutboundMined
            || cctx_status.status == CctxStatusStatus::Aborted
            || cctx_status.status == CctxStatusStatus::Reverted
        {
            tracing::debug!(
                "cctx {} is in final state, marking as processed",
                cctx.index
            );
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
        Ok(inserted)
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

        //insert unknown token if not present
        let unknown_token = TokenEntity::Entity::find()
            .filter(TokenEntity::Column::Zrc20ContractAddress.eq("UNKNOWN"))
            .one(self.db.as_ref())
            .await?;
        if unknown_token.is_none() {
            tracing::debug!("inserting unknown token");
            TokenEntity::Entity::insert(TokenEntity::ActiveModel {
                zrc20_contract_address: ActiveValue::Set("UNKNOWN".to_string()),
                asset: ActiveValue::Set("UNKNOWN".to_string()),
                foreign_chain_id: ActiveValue::Set(-9999),
                decimals: ActiveValue::Set(18),
                name: ActiveValue::Set("UNKNOWN".to_string()),
                symbol: ActiveValue::Set("UNKNOWN".to_string()),
                coin_type: ActiveValue::Set(DBCoinType::Gas),
                gas_limit: ActiveValue::Set("UNKNOWN".to_string()),
                paused: ActiveValue::Set(false),
                liquidity_cap: ActiveValue::Set("UNKNOWN".to_string()),
                created_at: ActiveValue::Set(Utc::now().naive_utc()),
                updated_at: ActiveValue::Set(Utc::now().naive_utc()),
                icon_url: ActiveValue::Set(None),
                ..Default::default()
            })
            .exec(self.db.as_ref())
            .await?;
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
        let inserted = self
            .batch_insert_transactions(job_id, &cctxs, &tx, None)
            .await?;
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

        let inserted = self
            .batch_insert_transactions(job_id, &cctxs, &tx, None)
            .await?;

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
        current_upper_bound_timestamp: Option<NaiveDateTime>,
    ) -> anyhow::Result<Vec<CctxListItemProto>> {
        let tx = self.db.begin().await?;
        let new_upper_bound_timstamp: i64 = cctxs
            .iter()
            .map(|cctx| {
                cctx.cctx_status
                    .last_update_timestamp
                    .parse::<i64>()
                    .unwrap_or_default()
            })
            .max()
            .unwrap_or_default();
        let new_upper_bound_timestamp =
            DateTime::<Utc>::from_timestamp(new_upper_bound_timstamp, 0)
                .unwrap()
                .naive_utc();
        let new_upper_bound_timestamp =
            new_upper_bound_timestamp.max(current_upper_bound_timestamp.unwrap_or_default());
        let imported = self
            .batch_insert_transactions(job_id, &cctxs, &tx, None)
            .await?;

        //Watermark with id 1 is the main historic sync thread, all others are one-offs that might be manually added to the db
        let new_status = if watermark_id != 1 {
            ProcessingStatus::Done
        } else {
            ProcessingStatus::Unlocked
        };
        tracing::debug!(
            "updating watermark : {:?} to pointer: {:?} and upper bound timestamp: {:?}",
            watermark_id,
            next_key,
            new_upper_bound_timestamp
        );
        self.update_watermark(
            watermark_id,
            next_key,
            Some(new_upper_bound_timestamp),
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
    pub async fn query_failed_cctxs(
        &self,
        batch_id: Uuid,
        batch_size: u32,
        polling_interval: u64,
    ) -> anyhow::Result<Vec<CctxShort>> {
        let statement = include_str!("query_for_update.sql").to_string();

        let statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            statement,
            vec![
                sea_orm::Value::String(Some(Box::new("Failed".to_string()))),
                sea_orm::Value::Int(Some(100)),
                sea_orm::Value::BigInt(Some(3600000)),
            ],
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
                    token_id: r.try_get_by_index(5)?,
                })
            })
            .collect::<Result<Vec<CctxShort>, sea_orm::DbErr>>();

        cctxs.map_err(|e| anyhow::anyhow!(e))
    }

    #[instrument(,level="trace",skip_all)]
    pub async fn get_unlocked_watermarks(
        &self,
        kind: Kind,
    ) -> anyhow::Result<Vec<UnlockedWatermark>> {
        let query_statement = r#"
            WITH unlocked_watermarks AS (
                SELECT id, pointer, retries_number, upper_bound_timestamp FROM watermark WHERE kind::text = $1 AND processing_status::text = 'Unlocked'
                FOR UPDATE SKIP LOCKED
            )
            UPDATE watermark
            SET processing_status = 'Locked' ,updated_at = NOW()
            WHERE id IN (SELECT id FROM unlocked_watermarks)
            RETURNING id, pointer, retries_number, upper_bound_timestamp
            "#;

        let query_statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            query_statement,
            vec![kind.to_string().into()],
        );

        let watermarks: Result<Vec<UnlockedWatermark>, sea_orm::DbErr> = self
            .db
            .query_all(query_statement)
            .await?
            .iter()
            .map(|r| {
                std::result::Result::Ok(UnlockedWatermark {
                    watermark_id: r.try_get_by_index(0)?,
                    pointer: r.try_get_by_index(1)?,
                    retries_number: r.try_get_by_index(2)?,
                    upper_bound_timestamp: r.try_get_by_index(3)?,
                })
            })
            .collect::<Result<Vec<UnlockedWatermark>, sea_orm::DbErr>>();

        watermarks.map_err(|e| anyhow::anyhow!(e))
    }

    #[instrument(,level="debug",skip(self), fields(batch_id = %batch_id))]
    pub async fn query_cctxs_for_status_update(
        &self,
        batch_size: u32,
        batch_id: Uuid,
        polling_interval: u64,
    ) -> anyhow::Result<Vec<CctxShort>> {
        // Optimized query that pre-calculates the backoff intervals to avoid complex expressions in WHERE clause
        let statement = include_str!("query_for_update.sql").to_string();

        let statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            statement,
            vec![
                sea_orm::Value::String(Some(Box::new("Unlocked".to_string()))),
                sea_orm::Value::Int(Some(batch_size as i32)),
                sea_orm::Value::BigInt(Some(polling_interval as i64)),
            ],
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
                    token_id: r.try_get_by_index(5)?,
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
            updated_by: ActiveValue::Set(job_id.to_string()),
            ..Default::default()
        })
        .filter(CrossChainTxEntity::Column::Id.eq(cctx_id))
        .exec(tx)
        .await?;
        Ok(())
    }

    async fn get_unknown_token(&self) -> anyhow::Result<TokenEntity::Model> {
        let unknown_token = TokenEntity::Entity::find()
            .filter(TokenEntity::Column::Zrc20ContractAddress.eq("UNKNOWN"))
            .one(self.db.as_ref())
            .await?
            .ok_or(anyhow::anyhow!("fallback token is absent"))?;
        Ok(unknown_token)
    }

    #[instrument(,level="trace",skip_all,fields(cctx_id = %cctx.index))]
    pub async fn calculate_token(
        &self,
        cctx: &CrossChainTx,
    ) -> anyhow::Result<Option<TokenEntity::Model>> {
        let unknown_token = self.get_unknown_token().await?;

        match cctx.inbound_params.coin_type {
            models::CoinType::NoAssetCall | models::CoinType::Cmd => return Ok(None),
            models::CoinType::Zeta => {
                let token = TokenEntity::Entity::find()
                    .filter(TokenEntity::Column::CoinType.eq(DBCoinType::Zeta))
                    .one(self.db.as_ref())
                    .await?
                    .unwrap_or(unknown_token);

                return Ok(Some(token));
            }
            models::CoinType::ERC20 => {
                let token = TokenEntity::Entity::find()
                    .filter(TokenEntity::Column::Asset.eq(&cctx.inbound_params.asset))
                    .one(self.db.as_ref())
                    .await?
                    .unwrap_or(unknown_token);
                return Ok(Some(token));
            }
            models::CoinType::Gas => {
                let chain_id = if cctx
                    .inbound_params
                    .sender_chain_id
                    .parse::<i32>()
                    .unwrap_or_default()
                    == self.zetachain_id
                {
                    &cctx.outbound_params[0].receiver_chain_id
                } else {
                    &cctx.inbound_params.sender_chain_id
                };
                let chain_id = chain_id.parse::<i32>().unwrap_or_default();
                let token = TokenEntity::Entity::find()
                    .filter(TokenEntity::Column::CoinType.eq(DBCoinType::Gas))
                    .filter(TokenEntity::Column::ForeignChainId.eq(chain_id))
                    .one(self.db.as_ref())
                    .await?
                    .unwrap_or(unknown_token);
                return Ok(Some(token));
            }
        }
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn update_cctx_status(
        &self,
        cctx: &CctxShort,
        fetched_cctx: CrossChainTx,
    ) -> anyhow::Result<CctxListItemProto> {
        let new_status: CctxStatusProto = fetched_cctx.cctx_status.status.clone().into();
        let new_status_db: CctxStatusStatus = fetched_cctx.cctx_status.status.into();
        let new_last_update_timestamp = chrono::DateTime::from_timestamp(
            fetched_cctx
                .cctx_status
                .last_update_timestamp
                .parse::<i64>()
                .unwrap_or(0),
            0,
        )
        .ok_or(anyhow::anyhow!("Invalid timestamp"))?
        .naive_utc();
        cctx_status::Entity::update_many()
            .set(cctx_status::ActiveModel {
                status: ActiveValue::Set(new_status_db),
                last_update_timestamp: ActiveValue::Set(new_last_update_timestamp),
                ..Default::default()
            })
            .filter(cctx_status::Column::CrossChainTxId.eq(cctx.id))
            .exec(self.db.as_ref())
            .await?;

        let first_outbound_params = fetched_cctx.outbound_params.first().unwrap();
        let token = if let Some(token_id) = cctx.token_id {
            TokenEntity::Entity::find()
                .filter(TokenEntity::Column::Id.eq(token_id))
                .one(self.db.as_ref())
                .await?
        } else {
            None
        };

        let cctx_list_item = CctxListItemProto {
            index: fetched_cctx.index,
            status: new_status.into(),
            status_reduced: reduce_status(new_status).into(),
            amount: fetched_cctx.inbound_params.amount,
            created_timestamp: fetched_cctx.cctx_status.created_timestamp.parse::<i64>()?,
            last_update_timestamp: fetched_cctx
                .cctx_status
                .last_update_timestamp
                .parse::<i64>()?,
            source_chain_id: fetched_cctx.inbound_params.sender_chain_id.parse::<i32>()?,
            target_chain_id: first_outbound_params.receiver_chain_id.parse::<i32>()?,
            sender_address: fetched_cctx.inbound_params.sender,
            receiver_address: first_outbound_params.receiver.clone(),
            asset: fetched_cctx.inbound_params.asset,
            coin_type: token
                .as_ref()
                .map(|t| t.coin_type.clone().into())
                .unwrap_or_default(),
            token_symbol: token.as_ref().map(|t| t.symbol.clone()),
            zrc20_contract_address: token.as_ref().map(|t| t.zrc20_contract_address.clone()),
            decimals: token.as_ref().map(|t| t.decimals as i64),
        };

        Ok(cctx_list_item)
    }

    pub async fn get_sync_progress(&self) -> anyhow::Result<SyncProgress> {
        let historical_watermark = watermark::Entity::find()
            .filter(
                sea_orm::Condition::all()
                    .add(watermark::Column::Kind.eq(Kind::Historical))
                    .add(watermark::Column::Id.eq(1)),
            )
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

        Ok(SyncProgress {
            historical_watermark_timestamp,
            pending_status_updates_count: 0,
            pending_cctxs_count: 0,
            realtime_gaps_count: realtime_gaps.len() as i64,
        })
    }

    #[instrument(level = "debug", skip_all, fields(index = %index))]
    pub async fn get_complete_cctx(
        &self,
        index: String,
    ) -> anyhow::Result<Option<CrossChainTxProto>> {
        let sql = include_str!("complete_cctx.sql");

        let statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            vec![sea_orm::Value::String(Some(Box::new(index.clone())))],
        );

        let cctx_rows = self.db.query_all(statement).await?;

        if cctx_rows.is_empty() {
            return Ok(None);
        }

        let cctx_row = &cctx_rows[0];

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

        let cctx_status: CctxStatusProto = cctx_row
            .try_get_by_index::<CctxStatusStatus>(15)
            .map_err(|e| anyhow::anyhow!("Invalid status: {}", e))?
            .into();
        // Build CctxStatus
        let status = StatusProto {
            status: cctx_status.into(),
            status_message: cctx_row
                .try_get_by_index(16)
                .map_err(|e| anyhow::anyhow!("cctx_row status_message: {}", e))?,
            error_message: cctx_row
                .try_get_by_index(17)
                .map_err(|e| anyhow::anyhow!("cctx_row error_message: {}", e))?,
            last_update_timestamp: last_update_timestamp.and_utc().timestamp(),
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

        let coin_type: CoinTypeProto = cctx_row
            .try_get_by_index::<DBCoinType>(28)
            .map_err(|e| anyhow::anyhow!("cctx_row invalid inbound_params coin_type: {}", e))?
            .into();
        let tx_finalization_status: TxFinalizationStatusProto = cctx_row
            .try_get_by_index::<TxFinalizationStatus>(35)
            .map_err(|e| {
                anyhow::anyhow!(
                    "cctx_row invalid inbound_params tx_finalization_status: {}",
                    e
                )
            })?
            .into();
        let inbound_status: InboundStatusProto = cctx_row
            .try_get_by_index::<InboundStatus>(37)
            .map_err(|e| anyhow::anyhow!("cctx_row invalid inbound_params inbound_status: {}", e))?
            .into();
        let confirmation_mode: ConfirmationModeProto = cctx_row
            .try_get_by_index::<ConfirmationMode>(38)
            .map_err(|e| {
                anyhow::anyhow!("cctx_row invalid inbound_params confirmation_mode: {}", e)
            })?
            .into();
        // Build InboundParams
        let inbound = InboundParamsProto {
            sender: cctx_row
                .try_get_by_index(25)
                .map_err(|e| anyhow::anyhow!("cctx_row sender: {}", e))?,
            sender_chain_id: cctx_row
                .try_get_by_index::<i32>(26)
                .map_err(|e| anyhow::anyhow!("cctx_row sender_chain_id: {}", e))?,
            tx_origin: cctx_row
                .try_get_by_index(27)
                .map_err(|e| anyhow::anyhow!("cctx_row tx_origin: {}", e))?,
            coin_type: coin_type.into(),
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
            tx_finalization_status: tx_finalization_status.into(),
            is_cross_chain_call: cctx_row
                .try_get_by_index(36)
                .map_err(|e| anyhow::anyhow!("cctx_row is_cross_chain_call: {}", e))?,
            status: inbound_status.into(),
            confirmation_mode: confirmation_mode.into(),
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
        let revert_options: Option<RevertOptionsProto> = {
            if cctx_status == CctxStatusProto::OutboundMined
                || cctx_status == CctxStatusProto::PendingOutbound
            {
                None
            } else {
                Some(RevertOptionsProto {
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
                        .try_get_by_index::<Option<String>>(44)
                        .map_err(|e| anyhow::anyhow!("cctx_row revert_message: {}", e))?,
                    revert_gas_limit: cctx_row
                        .try_get_by_index(45)
                        .map_err(|e| anyhow::anyhow!("cctx_row revert_gas_limit: {}", e))?,
                })
            }
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
        order by id ASC
        "#;

        let outbounds_statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            outbounds_sql,
            vec![sea_orm::Value::Int(Some(id))],
        );

        let outbounds_rows = self.db.query_all(outbounds_statement).await?;
        let mut outbounds = Vec::new();

        for outbound_row in outbounds_rows {
            let outbound_coin_type: CoinTypeProto = outbound_row
                .try_get_by_index::<DBCoinType>(4)
                .map_err(|e| {
                    anyhow::anyhow!("outbound_row invalid outbound_params coin_type: {}", e)
                })?
                .into();
            let outbound_tx_finalization_status: TxFinalizationStatusProto = outbound_row
                .try_get_by_index::<TxFinalizationStatus>(17)
                .map_err(|e| {
                    anyhow::anyhow!(
                        "outbound_row invalid outbound_params tx_finalization_status: {}",
                        e
                    )
                })?
                .into();
            let outbound_confirmation_mode: ConfirmationModeProto = outbound_row
                .try_get_by_index::<ConfirmationMode>(20)
                .map_err(|e| {
                    anyhow::anyhow!(
                        "outbound_row invalid outbound_params confirmation_mode: {}",
                        e
                    )
                })?
                .into();
            outbounds.push(OutboundParamsProto {
                receiver: outbound_row
                    .try_get_by_index::<String>(2)
                    .map_err(|e| anyhow::anyhow!("outbound_row receiver: {}", e))?,
                receiver_chain_id: outbound_row
                    .try_get_by_index::<i32>(3)
                    .map_err(|e| anyhow::anyhow!("outbound_row receiver_chain_id: {}", e))?,
                coin_type: outbound_coin_type.into(),
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
                    .map_err(|e| anyhow::anyhow!("outbound_row hash: {}", e))?,
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
                tx_finalization_status: outbound_tx_finalization_status.into(),
                call_options: Some(CallOptionsProto {
                    gas_limit: outbound_row
                        .try_get_by_index::<Option<String>>(18)
                        .map_err(|e| anyhow::anyhow!("outbound_row gas_limit: {}", e))?
                        .map(|x| x.parse::<u64>().unwrap_or(0)),
                    is_arbitrary_call: outbound_row
                        .try_get_by_index::<Option<bool>>(19)
                        .map_err(|e| anyhow::anyhow!("outbound_row is_arbitrary_call: {}", e))?,
                }),
                confirmation_mode: outbound_confirmation_mode.into(),
            });
        }

        let related_sql = include_str!("related.sql").to_string();

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
                .try_get_by_index(13)
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
            let inbound_coin_type: CoinTypeProto = related_row
                .try_get_by_index::<DBCoinType>(6)
                .map_err(|e| {
                    anyhow::anyhow!("related_row invalid inbound_params coin_type: {}", e)
                })?
                .into();

            let status: CctxStatusProto = related_row
                .try_get_by_index::<CctxStatusStatus>(3)
                .map_err(|e| anyhow::anyhow!("related_row status: {}", e))?
                .into();

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
                parent_index: None,
                inbound_amount: related_row
                    .try_get_by_index::<String>(5)
                    .map_err(|e| anyhow::anyhow!("related_row inbound_amount: {}", e))?,
                inbound_coin_type: inbound_coin_type.into(),
                inbound_asset: related_row
                    .try_get_by_index::<Option<String>>(7)
                    .map_err(|e| anyhow::anyhow!("related_row inbound_asset: {}", e))?,
                token_name: related_row
                    .try_get_by_index::<Option<String>>(8)
                    .map_err(|e| anyhow::anyhow!("related_row token_name: {}", e))?,
                token_symbol: related_row
                    .try_get_by_index::<Option<String>>(9)
                    .map_err(|e| anyhow::anyhow!("related_row token_symbol: {}", e))?,
                token_decimals: related_row
                    .try_get_by_index::<Option<i32>>(10)
                    .map_err(|e| anyhow::anyhow!("related_row token_decimals: {}", e))?,
                token_zrc20_contract_address: related_row
                    .try_get_by_index::<Option<String>>(11)
                    .map_err(|e| {
                        anyhow::anyhow!("related_row token_zrc20_contract_address: {}", e)
                    })?,
                token_icon_url: related_row
                    .try_get_by_index::<Option<String>>(12)
                    .map_err(|e| anyhow::anyhow!("related_row token_icon_url: {}", e))?,

                outbound_params,
            });
        }

        let status_reduced = reduce_status(CctxStatusProto::try_from(status.status)?);

        Ok(Some(CrossChainTxProto {
            creator,
            index,
            zeta_fees,
            relayed_message,
            cctx_status: Some(status),
            cctx_status_reduced: status_reduced.into(),
            inbound_params: Some(inbound),
            outbound_params: outbounds,
            revert_options,
            protocol_contract_version: protocol_contract_version.into(),
            related_cctxs: related,
            token_symbol,
            token_name,
            zrc20_contract_address,
            icon_url,
            decimals,
        }))
    }

    async fn prefetch_token_ids(&self, token_symbols: Vec<String>) -> anyhow::Result<Vec<i32>> {
        let token_ids = TokenEntity::Entity::find()
            .filter(TokenEntity::Column::Symbol.is_in(token_symbols))
            .all(self.db.as_ref())
            .await?;
        Ok(token_ids.into_iter().map(|x| x.id).collect())
    }

    fn get_status_by_reduced_status(&self, status_reduced: &str) -> Vec<String> {
        match status_reduced {
            "Pending" => vec!["PendingOutbound", "PendingInbound", "PendingRevert"]
                .into_iter()
                .map(|x| x.to_string())
                .collect(),
            "Success" => vec!["OutboundMined"]
                .into_iter()
                .map(|x| x.to_string())
                .collect(),
            "Failed" => vec!["Aborted", "Reverted"]
                .into_iter()
                .map(|x| x.to_string())
                .collect(),
            _ => vec![],
        }
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
            sql.push_str(&format!(
                " AND cs.status = ANY(${param_count}::cctx_status_status[])",
            ));
            params.push(sea_orm::Value::Array(
                sea_orm::sea_query::ArrayType::String,
                Some(Box::new(
                    filters
                        .status_reduced
                        .into_iter()
                        .flat_map(|s| self.get_status_by_reduced_status(&s))
                        .map(|s| sea_orm::Value::String(Some(Box::new(s))))
                        .collect::<Vec<_>>(),
                )),
            ));
        }

        if !filters.sender_address.is_empty() {
            param_count += 1;
            sql.push_str(&format!(" AND ip.sender = ANY(${param_count})"));
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
            sql.push_str(&format!(" AND cctx.receiver = ANY(${param_count})"));
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
            sql.push_str(&format!(" AND ip.asset = ANY(${param_count})"));
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
            sql.push_str(&format!(" AND ip.coin_type::text = ANY(${param_count})"));
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
            sql.push_str(&format!(" AND ip.sender_chain_id = ANY(${param_count})"));
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
            sql.push_str(&format!(
                " AND cctx.receiver_chain_id = ANY(${param_count})"
            ));
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
            sql.push_str(&format!(" AND cs.created_timestamp >= ${param_count}"));
            params.push(sea_orm::Value::BigInt(Some(start_timestamp)));
        }

        if let Some(end_timestamp) = filters.end_timestamp {
            param_count += 1;
            sql.push_str(&format!(" AND cs.created_timestamp <= ${param_count}"));
            params.push(sea_orm::Value::BigInt(Some(end_timestamp)));
        }

        if let Some(page_key) = page_key
            .and_then(|key| DateTime::<Utc>::from_timestamp(key, 0))
            .map(|dt| dt.naive_utc())
        {
            param_count += 1;
            if direction == Direction::Asc {
                sql.push_str(&format!(" AND last_update_timestamp > ${param_count}"));
            } else {
                sql.push_str(&format!(" AND last_update_timestamp < ${param_count}"));
            }
            params.push(sea_orm::Value::ChronoDateTime(Some(Box::new(page_key))));
        }

        if !filters.token_symbol.is_empty() {
            param_count += 1;
            let token_ids = self
                .prefetch_token_ids(filters.token_symbol.clone())
                .await?;
            sql.push_str(&format!(" AND cctx.token_id = ANY(${param_count})"));
            params.push(sea_orm::Value::Array(
                sea_orm::sea_query::ArrayType::Int,
                Some(Box::new(
                    token_ids
                        .into_iter()
                        .map(|s| sea_orm::Value::Int(Some(s)))
                        .collect::<Vec<_>>(),
                )),
            ));
        }

        if let Some(hash) = filters.hash {
            param_count += 1;
            sql.push_str(&format!(
                " AND (ip.observed_hash = ${param_count} OR ip.ballot_index = ${param_count})",
            ));
            params.push(sea_orm::Value::String(Some(Box::new(hash))));
        }

        // No need to group since we're only getting the last outbound_params row per CCTX
        sql.push(' ');

        // Add ordering and pagination
        param_count += 1;
        sql.push_str(&format!(
            " ORDER BY cs.last_update_timestamp {} LIMIT ${}",
            direction.as_str_name(),
            param_count
        ));
        params.push(sea_orm::Value::BigInt(Some(limit + 1)));

        let statement =
            Statement::from_sql_and_values(DbBackend::Postgres, sql.clone(), params.clone());

        let rows = self
            .db
            .query_all(statement.clone())
            .await
            .map_err(|e| anyhow::anyhow!("statement returned error: {}", e))?;

        let next_page_items_len = rows.len() - std::cmp::min(limit as usize, rows.len());
        let truncated = rows.iter().take(limit as usize);
        let timestamps = truncated.clone().map(|row| {
            row.try_get_by_index::<NaiveDateTime>(2)
                .map_err(|e| {
                    anyhow::anyhow!("row.try_get_by_index(2) last_update_timestamp: {}", e)
                })
                .unwrap_or_default()
        });
        let next_page_key = {
            if direction == Direction::Asc {
                timestamps.max()
            } else {
                timestamps.min()
            }
        };
        let next_page_key = next_page_key.unwrap_or_default().and_utc().timestamp();
        let mut items = Vec::new();
        for row in truncated {
            // Read the status enum directly from the database
            let index = row
                .try_get_by_index(0)
                .map_err(|e| anyhow::anyhow!("index: {}", e))?;
            let status: CctxStatusProto = row
                .try_get_by_index::<CctxStatusStatus>(1)
                .map_err(|e| anyhow::anyhow!("status: {}", e))?
                .into();

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
            let coin_type: CoinTypeProto = row
                .try_get_by_index::<DBCoinType>(10)
                .map_err(|e| anyhow::anyhow!("coin_type: {}", e))?
                .into();
            let status_reduced: CctxStatusReducedProto = reduce_status(status);
            let token_symbol: Option<String> = row
                .try_get_by_index(12)
                .map_err(|e| anyhow::anyhow!("token_symbol: {}", e))?;
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
                limit: next_page_items_len as i64,
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
        relation: Option<Relation>,
    ) -> anyhow::Result<Vec<CctxListItemProto>> {
        // Early return for empty input.
        if cctxs.is_empty() {
            return Ok(Vec::new());
        }

        // 1. Prepare and insert parent CrossChainTx records.
        let PreparedParentModels { models, token_map } =
            self.prepare_parent_models(job_id, cctxs, relation).await?;

        let inserted_cctxs = self.insert_parent_models(models).await?;

        // 2. Map fetched cctxs by index for quick access when preparing child records.
        let index_to_cctx = cctxs
            .iter()
            .map(|cctx| (cctx.index.as_str(), cctx))
            .collect::<std::collections::HashMap<_, _>>();

        // 3. Prepare child entity models and construct the protobuf response.
        let child_entities =
            Self::prepare_child_models(&inserted_cctxs, &index_to_cctx, &token_map)?;

        // 4. Persist child entities inside the provided transaction.
        self.insert_cctx_status_models(child_entities.status, tx)
            .await?;
        self.insert_inbound_params_models(child_entities.inbound_params, tx)
            .await?;
        self.insert_outbound_params_models(child_entities.outbound_params, tx)
            .await?;
        self.insert_revert_options_models(child_entities.revert_options, tx)
            .await?;

        Ok(child_entities.cctx_list_items)
    }

    async fn prepare_parent_models(
        &self,
        job_id: Uuid,
        cctxs: &Vec<CrossChainTx>,
        relation: Option<Relation>,
    ) -> anyhow::Result<PreparedParentModels> {
        let mut models = Vec::new();
        let mut token_map = HashMap::<String, Option<TokenEntity::Model>>::new();
        for cctx in cctxs {
            //if calculcate_token_id fails, we skip this cctx
            //if we fail insertion, the whole batch would be lost, which is not desired
            let token = match self.calculate_token(cctx).await {
                std::result::Result::Ok(value) => value,
                Err(e) => {
                    tracing::error!(
                        "Failed to calculate token id for cctx {}: {}",
                        cctx.index,
                        e
                    );
                    continue;
                }
            };
            token_map.insert(cctx.index.clone(), token.clone());

            let mut model = CrossChainTxEntity::ActiveModel {
                id: ActiveValue::NotSet,
                creator: ActiveValue::Set(cctx.creator.clone()),
                index: ActiveValue::Set(cctx.index.clone()),
                processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
                token_id: ActiveValue::Set(token.clone().map(|t| t.id)),
                zeta_fees: ActiveValue::Set(cctx.zeta_fees.clone()),
                receiver: ActiveValue::Set(Some(cctx.outbound_params[0].receiver.clone())),
                receiver_chain_id: ActiveValue::Set(Some(
                    cctx.outbound_params[0]
                        .receiver_chain_id
                        .clone()
                        .parse::<i32>()?,
                )),
                relayed_message: ActiveValue::Set(Some(cctx.relayed_message.clone())),
                protocol_contract_version: ActiveValue::Set(
                    ProtocolContractVersion::try_from(cctx.protocol_contract_version.clone())
                        .map_err(|e| anyhow::anyhow!(e))?,
                ),
                last_status_update_timestamp: ActiveValue::Set(Utc::now().naive_utc()),
                updated_by: ActiveValue::Set(job_id.to_string()),
                ..Default::default()
            };
            if let Some(relation) = relation {
                model.parent_id = ActiveValue::Set(Some(relation.parent_id));
                model.depth = ActiveValue::Set(relation.depth);
                model.root_id = ActiveValue::Set(Some(relation.root_id));
            }
            models.push(model);
        }
        Ok(PreparedParentModels { models, token_map })
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
                        CrossChainTxEntity::Column::TokenId,
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
        token_map: &HashMap<String, Option<TokenEntity::Model>>,
    ) -> anyhow::Result<ChildEntities> {
        let mut child_entities = ChildEntities {
            inbound_params: Vec::new(),
            outbound_params: Vec::new(),
            revert_options: Vec::new(),
            status: Vec::new(),
            cctx_list_items: Vec::new(),
        };

        for cctx in inserted_cctxs {
            if let Some(fetched_cctx) = index_to_cctx.get(cctx.index.as_str()) {
                // Prepare status model
                let token = token_map.get(cctx.index.as_str()).and_then(|t| t.as_ref());
                let last_update_timestamp = fetched_cctx
                    .cctx_status
                    .last_update_timestamp
                    .parse::<i64>()
                    .unwrap_or(0);
                let last_update_timestamp =
                    chrono::DateTime::<Utc>::from_timestamp(last_update_timestamp, 0)
                        .ok_or(anyhow::anyhow!("Invalid timestamp"))?
                        .naive_utc();

                child_entities.status.push(cctx_status::ActiveModel {
                    id: ActiveValue::NotSet,
                    cross_chain_tx_id: ActiveValue::Set(cctx.id),
                    status: ActiveValue::Set(CctxStatusStatus::from(
                        fetched_cctx.cctx_status.status.clone(),
                    )),
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
                child_entities
                    .inbound_params
                    .push(InboundParamsEntity::ActiveModel {
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
                    child_entities
                        .outbound_params
                        .push(OutboundParamsEntity::ActiveModel {
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
                            gas_priority_fee: ActiveValue::Set(Some(
                                outbound.gas_priority_fee.clone(),
                            )),
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
                            effective_gas_price: ActiveValue::Set(
                                outbound.effective_gas_price.clone(),
                            ),
                            effective_gas_limit: ActiveValue::Set(
                                outbound.effective_gas_limit.parse::<i64>()?,
                            ),
                            tss_pubkey: ActiveValue::Set(outbound.tss_pubkey.clone()),
                            tx_finalization_status: ActiveValue::Set(
                                TxFinalizationStatus::try_from(
                                    outbound.tx_finalization_status.clone(),
                                )
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
                child_entities
                    .revert_options
                    .push(RevertOptionsEntity::ActiveModel {
                        id: ActiveValue::NotSet,
                        cross_chain_tx_id: ActiveValue::Set(cctx.id),
                        revert_address: ActiveValue::Set(Some(
                            fetched_cctx.revert_options.revert_address.clone(),
                        )),
                        call_on_revert: ActiveValue::Set(
                            fetched_cctx.revert_options.call_on_revert,
                        ),
                        abort_address: ActiveValue::Set(Some(
                            fetched_cctx.revert_options.abort_address.clone(),
                        )),
                        revert_message: ActiveValue::Set(
                            fetched_cctx
                                .revert_options
                                .revert_message
                                .clone()
                                .map(sanitize_string),
                        ),
                        revert_gas_limit: ActiveValue::Set(
                            fetched_cctx.revert_options.revert_gas_limit.clone(),
                        ),
                    });

                let status: CctxStatusProto = fetched_cctx.cctx_status.clone().status.into();
                let coin_type_proto: CoinTypeProto =
                    fetched_cctx.inbound_params.coin_type.clone().into();
                // Prepare proto list item
                child_entities.cctx_list_items.push(CctxListItemProto {
                    index: cctx.index.clone(),
                    status: status.into(),
                    status_reduced: reduce_status(status).into(),
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
                    coin_type: coin_type_proto.into(),
                    token_symbol: token.map(|t| t.symbol.clone()),
                    zrc20_contract_address: token.map(|t| t.zrc20_contract_address.clone()),
                    decimals: token.map(|t| t.decimals as i64),
                });
            }
        }

        Ok(child_entities)
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
