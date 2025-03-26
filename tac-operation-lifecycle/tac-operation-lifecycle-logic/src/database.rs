use std::{cmp::min, collections::HashMap, sync::Arc, thread};
use anyhow::anyhow;
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ActiveValue::{self, NotSet}, ColumnTrait, Condition, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, Set, Statement, TransactionTrait
};
use tac_operation_lifecycle_entity::{interval, operation, operation_stage, stage_type, transaction, watermark};
use crate::client::models::operations::Operations as ApiOperations;

#[derive(Debug, Clone, Copy)]
pub enum OrderDirection {
    /// Process intervals from newest to oldest (for new jobs)
    LatestFirst,
    /// Process intervals from oldest to newest (for catch up)
    EarliestFirst,
}

impl OrderDirection {
    pub fn sql_order_string(&self) -> String {
        match self {
            OrderDirection::LatestFirst => "DESC".to_string(),
            OrderDirection::EarliestFirst => "ASC".to_owned(),
        }
    }
}

#[derive(Debug)]
pub enum EntityStatus {
    Pending,
    Processing,
    Finalized,
}

impl EntityStatus {
    pub fn to_id(&self) -> i16 {
        match self {
            EntityStatus::Pending => 0,
            EntityStatus::Processing => 1,
            EntityStatus::Finalized => 2,
        }
    }
    
    pub fn to_string(&self) -> String {
        format!("{:?}", self)
    }
}

pub struct TacDatabase {
    db: Arc<DatabaseConnection>,
}

impl TacDatabase {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    // Retrieves the saved watermark from the database (returns 0 if not exist)
    // NOTE: the method doesn't perform any cachingm just read from the DB!
    pub async fn get_watermark(&self) -> anyhow::Result<u64> {
        let existing_watermark = watermark::Entity::find()
            .one(self.db.as_ref())
            .await?;
        
        match  existing_watermark {
            Some(w) => Ok(w.timestamp as u64),
            _ => Ok(0),
        }
    }

    // Find and update existing watermark or create new one
    pub async fn set_watermark(&self, timestamp: u64) -> anyhow::Result<()> {
        let tx = self.db.begin().await?;
        self.set_watermark_internal(&tx, timestamp).await?;
        tx.commit().await?;

        Ok(())
    }

    async fn set_watermark_internal(&self, tx: &DatabaseTransaction, timestamp: u64) -> anyhow::Result<()> {
        let existing_watermark = watermark::Entity::find()
            .one(tx)
            .await?;

        let watermark_model = match existing_watermark {
            Some(w) => watermark::ActiveModel {
                id: ActiveValue::Unchanged(w.id),
                timestamp: ActiveValue::Set(timestamp as i64),
            },
            None => watermark::ActiveModel {
                id: ActiveValue::NotSet,
                timestamp: ActiveValue::Set(timestamp as i64),
            },
        };

        // Save the updated watermark
        watermark_model.save(tx).await?;

        Ok(())
    }

    // Create intervals with the specified period within the [from..to] boundary
    // Returns the number of created intervals
    // NOTE: the routine doesn't check if intervals will overlapped with existing ones
    pub async fn generate_pending_intervals(&self, from: u64, to: u64, period_secs: u64) -> anyhow::Result<usize> {
        const DB_BATCH_SIZE: usize = 1000;
        let intervals: Vec<interval::ActiveModel> = (from..to)
            .step_by(period_secs as usize)
            .map(|timestamp| interval::ActiveModel {
                id: ActiveValue::NotSet,
                start: ActiveValue::Set(timestamp as i64),
                //we don't want to save intervals that are out of specifiedboundaries
                end: ActiveValue::Set(min(timestamp + period_secs, to) as i64),
                timestamp: ActiveValue::Set(chrono::Utc::now().timestamp()),
                status:sea_orm::ActiveValue::Set(0 as i16),
                next_retry: ActiveValue::Set(None),
                retry_count: ActiveValue::Set(0 as i16),
            })
            .collect();

        tracing::info!("Total {}s intervals were generated: {} ({}..{})", period_secs, intervals.len(), from, to);

        // Process intervals in batches
        for chunk in intervals.chunks(DB_BATCH_SIZE) {
            let tx = self.db.begin().await?;
            
            match interval::Entity::insert_many(chunk.to_vec())
                .exec_with_returning(&tx)
                .await
            {
                Ok(_) => {
                    // Update watermark to the end of the last interval in this batch
                    let mut updated_watermark: Option<u64> = None;
                    if let Some(last_interval_from_batch) = chunk.last() {
                        updated_watermark = Some(last_interval_from_batch.end.clone().unwrap() as u64);
                        self.set_watermark_internal(&tx, updated_watermark.unwrap());
                    }
                    
                    tx.commit().await?; 
                    tracing::info!(
                        "Successfully saved batch of {} intervals and updated watermark to {}",
                        chunk.len(),
                        if let Some(wm) = updated_watermark { wm.to_string() } else { "[NOT_UPDATED]".to_string() },
                    );
                }
                Err(e) => {
                    tx.rollback().await?;
                    tracing::error!("Failed to save batch: {}", e);
                    return Err(e.into());
                }
            }
        }

        Ok(intervals.len())
    }

    // stage_types is a vector Id->StageName
    pub async fn register_stage_types(&self, stage_types: &HashMap<i32, String>) -> anyhow::Result<()> {
        for (stage_id, stage_name) in stage_types.into_iter() {
            let stage_type_model = stage_type::ActiveModel {
                id: Set(*stage_id),
                name: Set(stage_name.to_string()),
            };

            stage_type::Entity::insert(stage_type_model)
                .on_conflict(
                    OnConflict::column(stage_type::Column::Id)
                        .update_columns([stage_type::Column::Name])
                        .to_owned(),
                )
                .exec(self.db.as_ref())
                .await?;
        }

        Ok(())
    }

    pub async fn insert_pending_operations(&self, operations: &ApiOperations) -> anyhow::Result<()> {
        let thread_id = thread::current().id();
        
        // Start a transaction
        let txn = self.db.begin().await?;

        // Save all operations
        for op in operations {
            let operation_model = operation::ActiveModel {
                id: Set(op.id.clone()),
                operation_type: Set(None),
                timestamp: Set(op.timestamp as i64),
                status: Set(EntityStatus::Pending.to_id() as i32),
                next_retry: Set(None),
                retry_count: Set(0), // Initialize retry count
            };

            tracing::debug!("[Thread {:?}] Attempting to insert operation: {:?}", thread_id, operation_model);
            
            // Use on_conflict().do_nothing() with proper error handling
            match operation::Entity::insert(operation_model)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::column(operation::Column::Id)
                        .do_nothing()
                        .to_owned(),
                )
                .exec(&txn)
                .await {
                    Ok(_) => tracing::debug!("[Thread {:?}] Successfully inserted or skipped operation", thread_id),
                    Err(e) => {
                        tracing::debug!("[Thread {:?}] Error inserting operation: {:?}", thread_id, e);
                        // Don't fail the entire batch for a single operation
                        continue;
                    }
                }
        }

        // Commit transaction
        txn.commit().await?;

        Ok(())
    }

    pub async fn set_interval_status(&self, interval_id: i32, status: EntityStatus) -> anyhow::Result<()> {
        // Found record by PK
        match interval::Entity::find_by_id(interval_id).one(self.db.as_ref()).await? {
            Some(interval_model) => {
                let mut interval_active: interval::ActiveModel = interval_model.into();
                interval_active.status = Set(status.to_id());

                // Saving changes
                interval_active.update(self.db.as_ref()).await.map_err(|e| {
                    tracing::error!("Failed to update interval {} status to {}: {:?}", interval_id, status.to_string(), e);
                    anyhow!(e)
                })?;

                tracing::info!("Successfully updated interval {} status to {}", interval_id, status.to_string());
                Ok(())
            }
            None => {
                let err = anyhow!("Cannot update interval {} status to {}: not found", interval_id, status.to_string());
                tracing::error!("{}", err);
                Err(err)
            }
        }
    }

    pub async fn set_operation_status(&self, op_id: &String, status: EntityStatus) -> anyhow::Result<()> {
        // Found record by PK
        match operation::Entity::find_by_id(op_id).one(self.db.as_ref()).await? {
            Some(interval_model) => {
                let mut interval_active: operation::ActiveModel = interval_model.into();
                interval_active.status = Set(status.to_id() as i32);

                // Saving changes
                interval_active.update(self.db.as_ref()).await.map_err(|e| {
                    tracing::error!("Failed to update operation {} status to {}: {:?}", op_id, status.to_string(), e);
                    anyhow!(e)
                })?;

                tracing::info!("Successfully updated operation {} status to {}", op_id, status.to_string());
                Ok(())
            }
            None => {
                let err = anyhow!("Cannot update operation {} status to {}: not found", op_id, status.to_string());
                tracing::error!("{}", err);
                Err(err)
            }
        }
    }

    // Extract up to `num` intervals in the pending state with status switching to `processing`
    pub async fn pull_pending_intervals(&self, num: usize, order: OrderDirection) -> anyhow::Result<Vec<interval::Model>> {
        // Start transaction
        let txn = match self.db.begin().await {
            Ok(txn) => txn,
            Err(e) => {
                tracing::error!("Failed to begin transaction: {}", e);
                return Err(anyhow!(e));
            }
        };

        let stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            &format!(r#" SELECT id, start, "end", timestamp, status, next_retry, retry_count FROM interval WHERE status = 0 ORDER BY start {} LIMIT {} FOR UPDATE SKIP LOCKED "#, 
            order.sql_order_string(),
            num),
            vec![]
        );

        let pending_interval = match interval::Entity::find()
            .from_raw_sql(stmt)
            .one(&txn)
            .await {
                Ok(Some(interval)) => interval,
                Ok(None) => {
                    // No pending intervals, release transaction and return void array
                    let _ = txn.commit().await;

                    return Ok([].to_vec());
                },
                Err(e) => {
                    tracing::error!("Failed to fetch pending interval: {}", e);
                    let _ = txn.rollback().await;
                    
                    return Err(e.into());
                }
            };

        // Update the interval status to in-progress (1)
        let update_stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            r#" UPDATE interval SET status = 1 WHERE id = $1 RETURNING id, start, "end", timestamp, status, next_retry, retry_count "#,
            vec![pending_interval.id.into()]
        );

        let updated_interval = match interval::Entity::find()
            .from_raw_sql(update_stmt)
            .all(&txn)
            .await {
                Ok(updated_list) => {
                    // Commit the transaction before proceeding
                    if let Err(e) = txn.commit().await {
                        tracing::error!("Failed to commit transaction: {}", e);
                        
                        return Err(e.into());
                    }

                    updated_list
                }
                Ok(None) => {
                    tracing::error!("Failed to update interval: no rows returned");
                    let _ = txn.rollback().await;
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                Err(e) => {
                    tracing::error!("Failed to update interval: {}", e);
                    let _ = txn.rollback().await;
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };

        Ok([].to_vec())
    }
}