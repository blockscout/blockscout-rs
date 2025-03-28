use std::{cmp::min, collections::HashMap, sync::Arc, thread};
use anyhow::anyhow;
use sea_orm::{
    sea_query::{ArrayType, OnConflict},
    ActiveModelTrait,
    ActiveValue::{self, NotSet},
    ColumnTrait,
    DatabaseConnection,
    DatabaseTransaction,
    EntityTrait,
    QueryFilter,
    Set,
    Statement,
    TransactionTrait,
    Value,
};
use tac_operation_lifecycle_entity::{interval, operation, operation_stage, stage_type, transaction, watermark};
use crate::client::models::operations::Operations as ApiOperations;
use crate::client::models::profiling::OperationData as ApiOperationData;

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

    // Create interval with the specified boundary [from..to]
    // Update watermark in the database
    pub async fn generate_pending_interval(&self, from: u64, to: u64) -> anyhow::Result<()> {
        let interval = interval::ActiveModel {
            id: ActiveValue::NotSet,
            start: ActiveValue::Set(from as i64),
            end: ActiveValue::Set(to as i64),
            timestamp: ActiveValue::Set(chrono::Utc::now().timestamp()),
            status:sea_orm::ActiveValue::Set(EntityStatus::Pending.to_id()),
            next_retry: ActiveValue::Set(None),
            retry_count: ActiveValue::Set(0 as i16),
        };

        let tx = self.db.begin().await?;

        match interval::Entity::insert(interval)
            .exec(&tx)
            .await {
                Ok(_) => {
                    let _ = self.set_watermark_internal(&tx, to).await?;
                    
                    tx.commit().await?; 
                    tracing::info!( "Pending interval was added and watermark updated to {}", to);
                },

                Err(e) => {
                    tx.rollback().await?;
                    tracing::error!("Failed to add pending interval [{}..{}]: {}", from, to, e);
                    return Err(e.into());
                }
            };

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
                status:sea_orm::ActiveValue::Set(EntityStatus::Pending.to_id()),
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
                    // [assume bathes are sorted by `start` field ascending at that point]
                    let mut updated_watermark: Option<u64> = None;
                    if let Some(last_interval_from_batch) = chunk.last() {
                        updated_watermark = Some(last_interval_from_batch.end.clone().unwrap() as u64);
                        let _ = self.set_watermark_internal(&tx, updated_watermark.unwrap()).await;
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

    pub async fn set_interval_status(&self, interval_model: &interval::Model, status: EntityStatus) -> anyhow::Result<()> {
        let mut interval_active: interval::ActiveModel = interval_model.clone().into();
        interval_active.status = Set(status.to_id());

        // Saving changes
        interval_active.update(self.db.as_ref()).await.map_err(|e| {
            tracing::error!("Failed to update interval {} status to {}: {:?}", interval_model.id, status.to_string(), e);
            anyhow!(e)
        })?;

        //tracing::info!("Successfully updated interval {} status to {}", interval_id, status.to_string());
        Ok(())
    }

    pub async fn set_interval_status_by_id(&self, interval_id: i32, status: EntityStatus) -> anyhow::Result<()> {
        // Found record by PK
        match interval::Entity::find_by_id(interval_id).one(self.db.as_ref()).await? {
            Some(interval_model) => {
                self.set_interval_status(&interval_model, status).await
            }
            None => {
                let err = anyhow!("Cannot update interval {} status to {}: not found", interval_id, status.to_string());
                tracing::error!("{}", err);
                Err(err)
            }
        }
    }

    pub async fn set_operation_status(&self, operation_model: &operation::Model, status: EntityStatus) -> anyhow::Result<()> {
        let mut operation_active: operation::ActiveModel = operation_model.clone().into();
        operation_active.status = Set(status.to_id() as i32);

        // Saving changes
        operation_active.update(self.db.as_ref()).await.map_err(|e| {
            tracing::error!("Failed to update operation {} status to {}: {:?}", operation_model.id, status.to_string(), e);
            anyhow!(e)
        })?;

        tracing::info!("Successfully updated operation {} status to {}", operation_model.id, status.to_string());
        Ok(())
    }

    pub async fn set_operation_status_by_id(&self, op_id: &String, status: EntityStatus) -> anyhow::Result<()> {
        // Found record by PK
        match operation::Entity::find_by_id(op_id).one(self.db.as_ref()).await? {
            Some(operation_model) => {
                self.set_operation_status(&operation_model, status).await
            }
            None => {
                let err = anyhow!("Cannot update operation {} status to {}: not found", op_id, status.to_string());
                tracing::error!("{}", err);
                Err(err)
            }
        }
    }

    // Extract up to `num` intervals in the pending state and switch them status to `processing`
    pub async fn pull_pending_intervals(
        &self,
        num: usize,
        order: OrderDirection,
        from: Option<u64>,
        to: Option<u64>
    ) -> anyhow::Result<Vec<interval::Model>> {

        // prepare statement before starting transaction
        let mut conditions = vec![format!("status = {}", EntityStatus::Pending.to_id())];
        if let Some(start) = from {
            conditions.push(format!("start >= {}", start));
        }
        if let Some(end) = to {
            conditions.push(format!(r#""end" < {}"#, end));
        }
        let sql = format!(r#" SELECT id, start, "end", timestamp, status, next_retry, retry_count 
                                      FROM interval 
                                      WHERE {} 
                                      ORDER BY start {} 
                                      LIMIT {} 
                                      FOR UPDATE SKIP LOCKED "#, 
            conditions.join(" AND "),
            order.sql_order_string(),
            num
        );


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
            sql,
            vec![]
        );

        let pending_intervals = match interval::Entity::find()
            .from_raw_sql(stmt)
            .all(&txn)
            .await {
                Ok(intervals) => intervals,
                Err(e) => {
                    tracing::error!("Failed to fetch pending interval: {}", e);
                    let _ = txn.rollback().await;
                    
                    return Err(e.into());
                }
            };

        // Update the interval status to in-progress (1)
        let update_stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            r#"UPDATE interval SET status = $1 WHERE id = ANY($2) RETURNING id, start, "end", timestamp, status, next_retry, retry_count"#,
            vec![
                Value::SmallInt(Some(EntityStatus::Processing.to_id())),
                Value::Array(ArrayType::Int, Some(Box::new(pending_intervals.iter().map(|i| i.id.into()).collect()))),
            ]
        );

        match interval::Entity::find()
            .from_raw_sql(update_stmt)
            .all(&txn)
            .await {
                Ok(updated_list) => {
                    // Commit the transaction before proceeding
                    //let start = Instant::now();
                    if let Err(e) = txn.commit().await {
                        tracing::error!("Failed to pull intervals: {}", e);
                        
                        return Err(e.into());
                    }
                    //tracing::error!("Time taken to COMMIT {} INTERVALS: {} ms", pending_intervals.len(), start.elapsed().as_millis());

                    Ok(updated_list)
                }
                Err(e) => {
                    tracing::error!("Failed to pull intervals: {}", e);
                    let _ = txn.rollback().await;
                    
                    Err(e.into())
                }
            }
    }

    // Extract up to `num` operations in the pending state and switch them status to `processing`
    pub async fn pull_pending_operations(&self, num: usize, order: OrderDirection) -> anyhow::Result<Vec<operation::Model>> {
        // Start transaction
        let txn = match self.db.begin().await {
            Ok(txn) => txn,
            Err(e) => {
                tracing::error!("Failed to begin transaction: {}", e);
                return Err(anyhow!(e));
            }
        };

        // Find and lock pending operations
        let stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            format!(r#"SELECT id, operation_type,timestamp, next_retry,status, retry_count FROM operation WHERE status = {} ORDER BY timestamp {} LIMIT {} FOR UPDATE SKIP LOCKED"#,
                EntityStatus::Pending.to_id(),
                order.sql_order_string(),
                num
            ),
            vec![]
        );
        let pending_operations = match operation::Entity::find()
            .from_raw_sql(stmt)
            .all(&txn)
            .await {
                Ok(operations) => operations,
                Err(e) => {
                    tracing::error!("Failed to fetch pending operations: {}", e);
                    let _ = txn.rollback().await;
                    
                    return Err(e.into());
                }
            };

        // Update the operations status to in-progress (1)
        let update_stmt = Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            r#"UPDATE operation SET status = $1 WHERE id = ANY($2) RETURNING id, operation_type, timestamp, status, next_retry, retry_count"#,
            vec![
                Value::SmallInt(Some(EntityStatus::Processing.to_id())),
                Value::Array(ArrayType::String, Some(Box::new(pending_operations.iter().map(|o| o.id.clone().into()).collect()))),
            ]
        );

        match operation::Entity::find()
              .from_raw_sql(update_stmt)
              .all(&txn)
            .await {
                Ok(updated_list) => {
                    // Commit the transaction before proceeding
                    //let start = Instant::now();
                    if let Err(e) = txn.commit().await {
                        tracing::error!("Failed to pull operations: {}", e);
                        
                        return Err(e.into());
                    }
                    //tracing::error!("Time taken to COMMIT {} OPERATIONS: {} ms", pending_operations.len(), start.elapsed().as_millis());

                    Ok(updated_list)
                }
                Err(e) => {
                    tracing::error!("Failed to pull operations: {}", e);
                    let _ = txn.rollback().await;
                    
                    Err(e.into())
                }
            }
    }

    pub async fn set_operation_data(&self, operation: &operation::Model, operation_data: &ApiOperationData) -> anyhow::Result<()> {
        let txn = match self.db.begin().await {
            Ok(txn) => txn,
            Err(e) => {
                tracing::error!("Failed to begin transaction: {}", e);
                return Err(anyhow!(e));
            }
        };

        // Remove associated stages with transactions
        if let Err(e) = operation_stage::Entity::delete_many()
            .filter(operation_stage::Column::OperationId.eq(&operation.id))
            .exec(&txn)
            .await
        {
            tracing::error!("Failed to delete existing stages for operation {}: {}", operation.id, e);
            let _ = txn.rollback().await;
            return Err(e.into());
        }

        // Update operation type and status
        let mut operation_model: operation::ActiveModel = operation.clone().into();
        if operation_data.operation_type.is_finalized() {
            operation_model.status = Set(EntityStatus::Finalized.to_id().into());    // assume completed status
        }
        operation_model.operation_type = Set(Some(operation_data.operation_type.to_string()));
        
        if let Err(e) = operation_model.update(&txn).await {
            tracing::error!("Failed to update operation status: {}", e);
            let _ = txn.rollback().await;

            return Err(e.into());
        }

        // Store operation stages
        for (stage_type, stage_data) in operation_data.stages.iter() {
            if let Some(data) = &stage_data.stage_data {
                let stage_model = operation_stage::ActiveModel {
                    id: NotSet,
                    operation_id: Set(operation.id.clone()),
                    stage_type_id: Set(stage_type.to_id()),
                    success: Set(data.success),
                    timestamp: Set(data.timestamp as i64),
                    note: Set(data.note.clone()),
                };
                
                // TODO: Update operation_stage if existing
                match operation_stage::Entity::insert(stage_model)
                    .on_conflict(
                        sea_orm::sea_query::OnConflict::column(operation::Column::Id)
                            .do_nothing()
                            .to_owned(),
                    )
                    .exec_with_returning(&txn)
                    .await {
                        Ok(inserted_stage) => {
                            tracing::debug!("Successfully inserted stage {} for op_id {}", inserted_stage.stage_type_id, operation.id);

                            // TODO: Remove existing transactions for this stage first (selected by stage_id)

                            // store transactions for this stage
                            for tx in data.transactions.iter() {
                                let tx_model = transaction::ActiveModel {
                                    id: NotSet,
                                    stage_id: Set(inserted_stage.id),
                                    hash: Set(tx.hash.clone()),
                                    blockchain_type: Set(tx.blockchain_type.to_string()),
                                };

                                match transaction::Entity::insert(tx_model).exec(&txn).await {
                                    Ok(_) => tracing::debug!("Successfully inserted transaction for stage_id {}", inserted_stage.id),
                                    Err(e) => tracing::error!("Error inserting transaction: {:?}", e),
                                }
                            }

                        },
                        Err(e) => {
                            tracing::debug!("Error inserting stage: {:?}", e);
                            // Don't fail the entire batch for a single operation
                            continue;
                        }
                    }
            }
        }

        // Commit transaction
        match txn.commit().await {
            Ok(_) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn set_interval_retry(&self, interval: &interval::Model, retry_after_delay_sec: i64) -> anyhow::Result<()> {
        // Update interval with next retry timestamp and increment retry count
        let mut interval_model: interval::ActiveModel = interval.clone().into();
        interval_model.next_retry = Set(Some(chrono::Utc::now().timestamp() + retry_after_delay_sec));
        interval_model.retry_count = Set(interval.retry_count + 1);
        interval_model.status = Set(EntityStatus::Pending.to_id()); // Reset status to pending

        match interval_model.update(self.db.as_ref()).await {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!("Failed to update interval {} for retry: {}", interval.id, e);
                Err(e.into())
            }
        }
    }

    pub async fn set_operation_retry(&self, operation: &operation::Model, retry_after_delay_sec: i64) -> anyhow::Result<()> {
        // Update operation with next retry timestamp and increment retry count
        let mut operation_model: operation::ActiveModel = operation.clone().into();
        operation_model.next_retry = Set(Some(chrono::Utc::now().timestamp() + retry_after_delay_sec));
        operation_model.retry_count = Set(operation.retry_count + 1);
        operation_model.status = Set(EntityStatus::Pending.to_id().into()); // Reset status to pending

        match operation_model.update(self.db.as_ref()).await {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!("Failed to update operation {} for retry: {}", operation.id, e);
                Err(e.into())
            }
        }
    }
}