use std::{
    cmp::{max, min},
    sync::{atomic::AtomicU64, Arc}, time::Duration,
};

use anyhow::Error;
use client::{models::profiling::{BlockchainType, OperationType, StageType}, Client};
use reqwest;
use serde::{Deserialize, Serialize};
use sea_orm::{ActiveValue::{self, NotSet}, DatabaseConnection, EntityTrait, QueryFilter, Statement, TransactionTrait};
use tac_operation_lifecycle_entity::{interval, operation, operation_stage, stage_type, transaction, watermark};

pub mod client;
pub mod settings;

use futures::{
    future,
    stream::{self, repeat_with, select_with_strategy, BoxStream, PollNext},
    Stream, StreamExt,
};
use sea_orm::ColumnTrait;
use settings::IndexerSettings;
use tokio::time::sleep;
use chrono;
use tracing::{instrument, Instrument};
use sea_orm::ActiveModelTrait;
use sea_orm::Set;

#[derive(Debug, Clone)]
pub struct Job {
    pub interval: interval::Model,
}

#[derive(Debug, Clone)]
pub struct OperationJob {
    pub operation: operation::Model,
}

#[derive(Debug, Clone)]
pub enum IndexerJob {
    Interval(Job),
    Operation(OperationJob),
}

#[derive(Debug, Clone, Copy)]
pub enum OrderDirection {
    /// Process intervals from newest to oldest (for new jobs)
    Descending,
    /// Process intervals from oldest to newest (for catch up)
    Ascending,
}

impl OperationType {
    pub fn to_id(&self) -> i32 {
        match self {
            OperationType::Pending => 1,
            OperationType::TacTonTac => 2,
            OperationType::TacTon => 3,
            OperationType::TonTac => 4,
            OperationType::Rollback => 5,
            OperationType::Unknown => 6,
            OperationType::ErrorType => 0,
        }
    }

    pub fn to_string(&self) -> String { // SCREAMING_SNAKE_CASE
        serde_json::to_string(self)
            .unwrap()
            .trim_matches('"')
            .to_string()
    }

    pub fn is_finalized(&self) -> bool {
        self == &OperationType::Pending
    }
}

impl StageType {
    pub fn to_id(&self) -> i32 {
        match self {
            StageType::CollectedInTAC => 1,
            StageType::IncludedInTACConsensus => 2,
            StageType::ExecutedInTAC => 3,
            StageType::CollectedInTON => 4,
            StageType::IncludedInTONConsensus => 5,
            StageType::ExecutedInTON => 6,
        }
    }

    pub fn to_string(&self) -> String {
        format!("{:?}", self)
    }
}

impl BlockchainType {
    pub fn to_string(&self) -> String {
        format!("{:?}", self)
    }
}

pub struct Indexer {
    client: Client,
    settings: IndexerSettings,
    watermark: AtomicU64,
    db: Arc<DatabaseConnection>,
}

impl Indexer {
    pub async fn new(
        settings: IndexerSettings,
        db: Arc<DatabaseConnection>,
    ) -> anyhow::Result<Self> {
        
        let watermark = watermark::Entity::find()
            .one(db.as_ref())
            .await?
            .map_or(settings.start_timestamp as u64, |row| {
                max(row.timestamp as u64, settings.start_timestamp as u64)
            });
        Ok(Self {
            client: Client::new(settings.client.clone()),
            settings,
            watermark: AtomicU64::new(watermark),
            db,
        })
    }
}

impl Indexer {
    pub async fn find_inconsistent_intervals(&self) -> anyhow::Result<()> {
        let intervals = interval::Entity::find()
            .filter(
                interval::Column::Start
                    .gt(self.watermark.load(std::sync::atomic::Ordering::Acquire) as i64),
            )
            .all(self.db.as_ref())
            .await?;

        for interval in intervals {
            tracing::error!("Interval {} is inconsistent", interval.id);
        }
        Ok(())
    }

    //generate intervals between current epoch and watermark and save them to db
    #[instrument(name = "save_intervals", skip_all, level = "info")]
    pub async fn save_intervals(&self,now: u64) -> anyhow::Result<usize> {
        let time_interval = self.settings.catchup_interval.as_secs();
        let watermark = self.watermark.load(std::sync::atomic::Ordering::Acquire);
        let batch_size = 1000; // Adjust this based on your needs and database performance
        let catch_up_interval = time_interval;
        let intervals: Vec<interval::ActiveModel> = (watermark as u64..now)
            .step_by(catch_up_interval as usize)
            .map(|timestamp| interval::ActiveModel {
                start: ActiveValue::Set(timestamp as i64),
                //we don't want to save intervals that are in the future
                end: ActiveValue::Set(min(timestamp + catch_up_interval, now) as i64),
                timestamp: ActiveValue::Set(timestamp as i64),
                id: ActiveValue::NotSet,
                status:sea_orm::ActiveValue::Set(0 as i16),
                next_retry: ActiveValue::Set(None),
                retry_count: ActiveValue::Set(0 as i16),
            })
            .collect();

        tracing::info!("Total {}s intervals to save: {} ({}..{})", time_interval, intervals.len(), watermark, now);

        // Process intervals in batches
        for chunk in intervals.chunks(batch_size) {
            let tx = self.db.begin().await?;
            
            match interval::Entity::insert_many(chunk.to_vec())
                .exec_with_returning(&tx)
                .await
            {
                Ok(_) => {
                    // Update watermark to the end of the last interval in this batch
                    
                    // Find existing watermark or create new one
                    let existing_watermark = watermark::Entity::find()
                        .one(&tx)
                        .await?;

                    let watermark_model = match existing_watermark {
                        Some(w) => watermark::ActiveModel {
                            id: ActiveValue::Unchanged(w.id),
                            timestamp: ActiveValue::Set(now as i64),
                        },
                        None => watermark::ActiveModel {
                            id: ActiveValue::NotSet,
                            timestamp: ActiveValue::Set(now as i64),
                        },
                    };

                    // Save the updated watermark
                    watermark_model.save(&tx).await?;
                    
                    // Update the in-memory watermark
                    
                    self.watermark.store(now, std::sync::atomic::Ordering::Release);
                    
                    tx.commit().await?; 
                    tracing::info!(
                        "Successfully saved batch of {} intervals and updated watermark to {}",
                        chunk.len(),
                        now
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

    pub async fn ensure_stages_types_exist(&self) -> Result<(), sea_orm::DbErr> {
        let mut num_stages_added = 0;
        for op_type in [
            StageType::CollectedInTAC,
            StageType::IncludedInTACConsensus,
            StageType::ExecutedInTAC,
            StageType::CollectedInTON,
            StageType::IncludedInTONConsensus,
            StageType::ExecutedInTON,
        ] {
            let stage_id = op_type.to_id();
            let stage_name = op_type.to_string();
    
            if stage_type::Entity::find()
                .filter(stage_type::Column::Id.eq(stage_id))
                .one(self.db.as_ref())
                .await?
                .is_none()
            {
                let new_stage = stage_type::ActiveModel {
                    id: Set(stage_id),
                    name: Set(stage_name),
                };
                new_stage.insert(self.db.as_ref()).await?;

                num_stages_added = num_stages_added + 1;
            }
        }

        if num_stages_added > 0 {
            tracing::info!("Added {} stage types to the database", num_stages_added);
        }

        Ok(())
    }

    pub fn watermark(&self) -> u64 {
        self.watermark.load(std::sync::atomic::Ordering::Acquire)
    }
    

    pub async fn fetch_operations(&self, job: &Job) -> Result<usize, Error> {
        use sea_orm::Set;
        use tac_operation_lifecycle_entity::{interval, operation};
        use std::thread;

        let thread_id = thread::current().id();
        tracing::debug!("[Thread {:?}] Processing interval job: {:?}", thread_id, job);

        let operations = self.client.get_operations(job.interval.start as u64, job.interval.end as u64).await?;
        let ops_num = operations.len();
        
        if ops_num > 0 {
            tracing::info!("[Thread {:?}] Fetched {} op_ids: {:#?}", thread_id, ops_num, operations);
        }
        
        // Start a transaction
        let txn = self.db.begin().await?;

        // Save all operations
        for op in operations {
            let operation_model = operation::ActiveModel {
                id: Set(op.id),
                operation_type: Set(None),
                timestamp: Set(op.timestamp as i64),
                status: Set(0), // Status 0 means pending
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

        // Update interval status to finished (2)
        let mut interval_model: interval::ActiveModel = job.interval.clone().into();
        interval_model.status = Set(2);
        
        interval_model.update(&txn).await?;

        // Commit transaction
        txn.commit().await?;

        tracing::info!("[Thread {:?}] Successfully processed job: id={}, start={}, end={}", 
            thread_id, job.interval.id, job.interval.start, job.interval.end);

        Ok(ops_num)
    }

    pub fn interval_stream(&self, direction: OrderDirection) -> BoxStream<'_, IndexerJob> {
        use sea_orm::Statement;
        use std::time::{SystemTime, UNIX_EPOCH};
        
        Box::pin(async_stream::stream! {
            loop {
                // Start transaction
                let txn = match self.db.begin().await {
                    Ok(txn) => txn,
                    Err(e) => {
                        tracing::error!("Failed to begin transaction: {}", e);
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        continue;
                    }
                };

                // Find and lock a single pending interval with specified ordering
                let order_by = match direction {
                    OrderDirection::Descending => "DESC",
                    OrderDirection::Ascending => "ASC",
                };

                let stmt = Statement::from_sql_and_values(
                    sea_orm::DatabaseBackend::Postgres,
                    &format!(r#" SELECT id, start, "end", timestamp, status, next_retry, retry_count FROM interval WHERE status = 0 ORDER BY start {} LIMIT 1 FOR UPDATE SKIP LOCKED "#, 
                    order_by),
                    vec![]
                );

                let pending_interval = match interval::Entity::find()
                    .from_raw_sql(stmt)
                    .one(&txn)
                    .await {
                        Ok(Some(interval)) => interval,
                        Ok(None) => {
                            // No pending intervals, release transaction and wait
                            let _ = txn.commit().await;
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        },
                        Err(e) => {
                            tracing::error!("Failed to fetch pending interval: {}", e);
                            let _ = txn.rollback().await;
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
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
                    .one(&txn)
                    .await {
                        Ok(Some(updated)) => {
                            // Commit the transaction before proceeding
                            if let Err(e) = txn.commit().await {
                                tracing::error!("Failed to commit transaction: {}", e);
                                continue;
                            }
                            updated
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

                let job = Job {
                    interval: updated_interval.clone(),
                };

                // Yield the job and continue
                yield IndexerJob::Interval(job.clone());
                
                // Sleep a bit before next iteration to prevent tight loop
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
    }

    pub fn operations_stream(&self) -> BoxStream<'_, IndexerJob> {
        const LOOP_DELAY_INTERVAL_MS: u64 = 2000;
        Box::pin(async_stream::stream! {
            loop {
                // Start transaction
                let txn = match self.db.begin().await {
                    Ok(txn) => txn,
                    Err(e) => {
                        tracing::error!("Failed to begin transaction: {}", e);
                        tokio::time::sleep(Duration::from_millis(LOOP_DELAY_INTERVAL_MS)).await;
                        continue;
                    }
                };

                // Find and lock a single pending operation
                let stmt = Statement::from_sql_and_values(
                    sea_orm::DatabaseBackend::Postgres,
                    r#"SELECT id, operation_type,timestamp, next_retry,status, retry_count FROM operation WHERE status = 0 ORDER BY timestamp ASC LIMIT 1 FOR UPDATE SKIP LOCKED"#,
                    vec![]
                );

                let pending_operation = match operation::Entity::find()
                    .from_raw_sql(stmt)
                    .one(&txn)
                    .await {
                        Ok(Some(operation)) => operation,
                        Ok(None) => {
                            // No pending operations, release transaction and wait
                            let _ = txn.commit().await;
                            tokio::time::sleep(Duration::from_millis(LOOP_DELAY_INTERVAL_MS)).await;
                            continue;
                        },
                        Err(e) => {
                            tracing::error!("Failed to fetch pending operation: {}", e);
                            let _ = txn.rollback().await;
                            tokio::time::sleep(Duration::from_millis(LOOP_DELAY_INTERVAL_MS)).await;
                            continue;
                        }
                    };

                // Update the operation status to in-progress (1)
                let update_stmt = Statement::from_sql_and_values(
                    sea_orm::DatabaseBackend::Postgres,
                    r#"UPDATE operation SET status = 1 WHERE id = $1 RETURNING id, operation_type, timestamp, status, next_retry, retry_count"#,
                    vec![pending_operation.id.into()]
                );

                match operation::Entity::find()
                    .from_raw_sql(update_stmt)
                    .one(&txn)
                    .await {
                        Ok(Some(updated)) => {
                            // Commit the transaction before yielding
                            if let Err(e) = txn.commit().await {
                                tracing::error!("Failed to commit transaction: {}", e);
                                continue;
                            }

                            yield IndexerJob::Operation(OperationJob {
                                operation: updated,
                            });
                        }
                        Ok(None) => {
                            tracing::error!("Failed to update operation: no rows returned");
                            let _ = txn.rollback().await;
                            tokio::time::sleep(Duration::from_millis(LOOP_DELAY_INTERVAL_MS)).await;
                            continue;
                        }
                        Err(e) => {
                            tracing::error!("Failed to update operation: {}", e);
                            let _ = txn.rollback().await;
                            tokio::time::sleep(Duration::from_millis(LOOP_DELAY_INTERVAL_MS)).await;
                            continue;
                        }
                    }
            }
        })
    }

    pub async fn process_operation_with_retries(&self, jobs: Vec<&OperationJob>) -> () {
        use sea_orm::Set;
        use std::time::{SystemTime, UNIX_EPOCH};

        let op_ids: Vec<&str> = jobs.iter().map(|j| j.operation.id.as_str()).collect();

        match self.client.get_operations_stages(op_ids.clone()).await {
            Ok(operations_map) => {
                // Start a transaction
                let txn = match self.db.begin().await {
                    Ok(txn) => txn,
                    Err(e) => {
                        tracing::error!("Failed to begin transaction: {}", e);
                        return;
                    }
                };

                for (op_id, operation_data) in operations_map.iter() {
                    // Find an associated operation in the input operations vector
                    match jobs.iter().find(|j| &j.operation.id == op_id) {
                        Some(job) => {
                            // Update operation type and status
                            let mut operation_model: operation::ActiveModel = job.operation.clone().into();
                            if operation_data.operationType.is_finalized() {
                                operation_model.status = Set(2);    // assume completed status
                            }
                            operation_model.operation_type = Set(Some(operation_data.operationType.to_string()));
                            
                            if let Err(e) = operation_model.update(&txn).await {
                                tracing::error!("Failed to update operation status: {}", e);
                                let _ = txn.rollback().await;
                                return;
                            }

                            // Store operation stages
                            for (stage_type, stage_data) in operation_data.stages.iter() {
                                if let Some(data) = &stage_data.stage_data {
                                    let stage_model = operation_stage::ActiveModel {
                                        id: NotSet,
                                        operation_id: Set(op_id.to_string()),
                                        stage_type_id: Set(stage_type.to_id()),
                                        success: Set(data.success),
                                        timestamp: Set(data.timestamp as i64),
                                        note: Set(data.note.clone()),
                                    };
                                    
                                    match operation_stage::Entity::insert(stage_model)
                                        .on_conflict(
                                            sea_orm::sea_query::OnConflict::column(operation::Column::Id)
                                                .do_nothing()
                                                .to_owned(),
                                        )
                                        .exec_with_returning(&txn)
                                        .await {
                                            Ok(inserted_stage) => {
                                                tracing::debug!("Successfully inserted stage for op_id {}", op_id);

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
                        },
                        None => {
                            tracing::error!("Stage profiling response contains unknown operation (id = {}). Skipping...", op_id);
                        }
                    }
                }

                // Commit transaction
                if let Err(e) = txn.commit().await {
                    tracing::error!("Failed to commit transaction: {}", e);
                    return;
                }

                tracing::info!("Successfully processed operations: id={}", op_ids.join(","));
            }
            _ => {
                let _ = jobs.iter().map(|job| async {
                    let retries = job.operation.retry_count;
                    let base_delay = 5; // 5 seconds base delay
                    let next_retry = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64 + (base_delay * 2i64.pow(retries as u32)) as i64;

                    // Update operation with next retry timestamp and increment retry count
                    let mut operation_model: operation::ActiveModel = job.operation.clone().into();
                    operation_model.next_retry = Set(Some(next_retry));
                    operation_model.retry_count = Set(retries + 1);
                    operation_model.status = Set(0); // Keep status as pending

                    if let Err(update_err) = operation_model.update(self.db.as_ref()).await {
                        tracing::error!("Failed to update operation next_retry: {}", update_err);
                    }
                });
            }
        }
    }

    fn prio_left(_: &mut ()) -> PollNext { PollNext::Left }

    #[instrument(name = "indexer", skip_all, level = "info")]
    pub async fn start(&self) -> anyhow::Result<()> {
        use futures::stream::{select_with_strategy, PollNext};

        self.ensure_stages_types_exist().await?;

        tracing::warn!("starting indexer");
        self.save_intervals(chrono::Utc::now().timestamp() as u64).await?;
        
        // Create prioritized streams
        let high_priority = self.interval_stream(OrderDirection::Descending);
        let low_priority = self.interval_stream(OrderDirection::Ascending);
        let operations = self.operations_stream();

        // Combine streams with prioritization (high priority first)
        let interval_stream = select_with_strategy(high_priority, low_priority, Self::prio_left);
        let mut combined_stream = select_with_strategy(operations, interval_stream, Self::prio_left);

        // Process the prioritized stream
        while let Some(job) = combined_stream.next().await {
            match job {
                IndexerJob::Interval(job) => {
                    match self.fetch_operations(&job).await {
                        Ok(num) => {
                            if num > 0 {
                                tracing::info!("Successfully fetched {} operations for interval {}", num, job.interval.id);
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch operations: {}", e);
                            
                            // Schedule retry with exponential backoff
                            let retries = job.interval.retry_count;
                            let base_delay = 5; // 5 seconds base delay
                            let next_retry = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs() as i64 + (base_delay * 2i64.pow(retries as u32)) as i64;

                            // Update interval with next retry timestamp and increment retry count
                            let mut interval_model: interval::ActiveModel = job.interval.into();
                            interval_model.next_retry = Set(Some(next_retry));
                            interval_model.retry_count = Set(retries + 1);
                            interval_model.status = Set(0); // Reset status to pending

                            if let Err(update_err) = interval_model.update(self.db.as_ref()).await {
                                tracing::error!("Failed to update interval for retry: {}", update_err);
                            }   
                        }
                    }
                }
                IndexerJob::Operation(job) => {
                    self.process_operation_with_retries([&job].to_vec()).await;
                }
            }
        }

        Ok(())
    }
}
