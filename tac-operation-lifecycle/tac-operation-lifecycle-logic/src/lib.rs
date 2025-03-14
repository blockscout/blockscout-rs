use std::{
    cmp::{max, min},
    sync::{atomic::AtomicU64, Arc}, time::Duration,
};

use anyhow::Error;
use client::Client;
use reqwest;
use serde::{Deserialize, Serialize};
use sea_orm::{Statement, ActiveValue, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait};
use tac_operation_lifecycle_entity::{interval, operation, watermark};

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
    pub async fn save_intervals(&self) -> anyhow::Result<()> {
        let now = chrono::Utc::now().timestamp() as u64;
        let watermark = self.watermark.load(std::sync::atomic::Ordering::Acquire);
        let batch_size = 1000; // Adjust this based on your needs and database performance
        let catch_up_interval = self.settings.catchup_interval.as_secs() as u64;
        let intervals: Vec<interval::ActiveModel> = (watermark as u64..now)
            .step_by(catch_up_interval as usize)
            .map(|timestamp| interval::ActiveModel {
                start: ActiveValue::Set(timestamp as i64),
                //we don't want to save intervals that are in the future
                end: ActiveValue::Set(min(timestamp + catch_up_interval, now) as i64),
                timestamp: ActiveValue::Set(chrono::Utc::now().timestamp()),
                id: ActiveValue::NotSet,
                status:sea_orm::ActiveValue::Set(0 as i16),
                next_retry: ActiveValue::Set(0 as i64),
                retry_count: ActiveValue::Set(0 as i16),
            })
            .collect();

        tracing::info!("Total intervals to save: {}", intervals.len());

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

        Ok(())
    }

    pub fn watermark(&self) -> u64 {
        self.watermark.load(std::sync::atomic::Ordering::Acquire)
    }
    

    pub async fn fetch_operations(&self, job: &Job) -> () {
        use sea_orm::Set;
        use tac_operation_lifecycle_entity::{interval, operation};
        use std::time::{SystemTime, UNIX_EPOCH};

        match self.client.get_operations(job.interval.start as u64, job.interval.end as u64).await {
            Ok(operations) => {
                // Start a transaction
                let txn = match self.db.begin().await {
                    Ok(txn) => txn,
                    Err(e) => {
                        tracing::error!("Failed to begin transaction: {}", e);
                        return;
                    }
                };

                // Save all operations
                for op in operations {
                    let operation_model = operation::ActiveModel {
                        operation_id: Set(op.operation_id.parse().unwrap_or(0)),
                        timestamp: Set(op.timestamp as i64),
                        status: Set(0), // Status 0 means pending
                        next_retry: Set(None),
                        retry_count: Set(0), // Initialize retry count
                    };

                    if let Err(e) = operation_model.insert(&txn).await {
                        tracing::error!("Failed to insert operation: {}", e);
                        let _ = txn.rollback().await;
                        return;
                    }
                }

                // Update interval status to finished (2)
                let mut interval_model: interval::ActiveModel = job.interval.clone().into();
                interval_model.status = Set(2);
                
                if let Err(e) = interval_model.update(&txn).await {
                    tracing::error!("Failed to update interval status: {}", e);
                    let _ = txn.rollback().await;
                    return;
                }

                // Commit transaction
                if let Err(e) = txn.commit().await {
                    tracing::error!("Failed to commit transaction: {}", e);
                    return;
                }

                tracing::info!("Successfully processed job: id={}, start={}, end={}", 
                    job.interval.id, job.interval.start, job.interval.end);
            }
            Err(e) => {
                tracing::error!("Failed to get operations: {}", e);
                
                let retries = job.interval.retry_count;
                let base_delay = 5; // 5 seconds base delay
                let next_retry = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64 + (base_delay * 2i64.pow(retries as u32)) as i64;

                // Update interval with next retry timestamp and increment retry count
                let mut interval_model: interval::ActiveModel = job.interval.clone().into();
                interval_model.next_retry = Set(next_retry);
                interval_model.retry_count = Set(retries + 1);
                interval_model.status = Set(0); // Keep status as pending

                if let Err(update_err) = interval_model.update(self.db.as_ref()).await {
                    tracing::error!("Failed to update interval next_retry: {}", update_err);
                }
            }
        }
    }

    pub fn fetch_interval_stream(&self, direction: OrderDirection) -> BoxStream<'_, IndexerJob> {
        use sea_orm::Statement;
        
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
                    &format!(r#"
                    SELECT id, start, "end", timestamp, status, next_retry
                    FROM interval
                    WHERE status = 0
                    ORDER BY start {}
                    LIMIT 1
                    FOR UPDATE SKIP LOCKED
                    "#, order_by),
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

                // Update the interval status
                let update_stmt = Statement::from_sql_and_values(
                    sea_orm::DatabaseBackend::Postgres,
                    r#"
                    UPDATE interval
                    SET status = 1
                    WHERE id = $1
                    RETURNING id, start, "end", timestamp, status, next_retry
                    "#,
                    vec![pending_interval.id.into()]
                );

                match interval::Entity::find()
                    .from_raw_sql(update_stmt)
                    .one(&txn)
                    .await {
                        Ok(Some(updated)) => {
                            // Commit the transaction before yielding
                            if let Err(e) = txn.commit().await {
                                tracing::error!("Failed to commit transaction: {}", e);
                                continue;
                            }

                            yield IndexerJob::Interval(Job {
                                interval: updated,
                            });
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
                    }
            }
        })
    }

    pub fn fetch_operation_stream(&self) -> BoxStream<'_, IndexerJob> {
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

                // Find and lock a single pending operation
                let stmt = Statement::from_sql_and_values(
                    sea_orm::DatabaseBackend::Postgres,
                    r#"
                    SELECT operation_id, timestamp, status, next_retry
                    FROM operation
                    WHERE status = 0
                    ORDER BY timestamp ASC
                    LIMIT 1
                    FOR UPDATE SKIP LOCKED
                    "#,
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
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        },
                        Err(e) => {
                            tracing::error!("Failed to fetch pending operation: {}", e);
                            let _ = txn.rollback().await;
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }
                    };

                // Update the operation status to in-progress (1)
                let update_stmt = Statement::from_sql_and_values(
                    sea_orm::DatabaseBackend::Postgres,
                    r#"
                    UPDATE operation
                    SET status = 1
                    WHERE operation_id = $1
                    RETURNING operation_id, timestamp, status, next_retry
                    "#,
                    vec![pending_operation.operation_id.into()]
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
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }
                        Err(e) => {
                            tracing::error!("Failed to update operation: {}", e);
                            let _ = txn.rollback().await;
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }
                    }
            }
        })
    }

    pub async fn process_operation_with_retries(&self, job: &OperationJob) -> () {
        use sea_orm::Set;
        use std::time::{SystemTime, UNIX_EPOCH};

        let client = reqwest::Client::new();
        let request_body = serde_json::json!({
            "operationIds": [format!("0x{:x}", job.operation.operation_id)]
        });

        match client
            .post("https://data.turin.tac.build/stage-profiling")
            .header("accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await {
                Ok(response) if response.status().is_success() => {
                    // Start a transaction
                    let txn = match self.db.begin().await {
                        Ok(txn) => txn,
                        Err(e) => {
                            tracing::error!("Failed to begin transaction: {}", e);
                            return;
                        }
                    };

                    // Update operation status to completed (2)
                    let mut operation_model: operation::ActiveModel = job.operation.clone().into();
                    operation_model.status = Set(2);
                    
                    if let Err(e) = operation_model.update(&txn).await {
                        tracing::error!("Failed to update operation status: {}", e);
                        let _ = txn.rollback().await;
                        return;
                    }

                    // Commit transaction
                    if let Err(e) = txn.commit().await {
                        tracing::error!("Failed to commit transaction: {}", e);
                        return;
                    }

                    tracing::info!("Successfully processed operation: id={}", job.operation.operation_id);
                }
                _ => {
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
                }
            }
    }

    #[instrument(name = "indexer", skip_all, level = "info")]
    pub async fn start(&self) -> anyhow::Result<()> {
        tracing::warn!("starting indexer");
        self.save_intervals().await?;
        
        let mut stream = stream::SelectAll::<BoxStream<'_, IndexerJob>>::new();
        stream.push(self.fetch_interval_stream(OrderDirection::Ascending)); // For catch up
        stream.push(self.fetch_interval_stream(OrderDirection::Descending)); // For new jobs
        stream.push(self.fetch_operation_stream()); // Add operation stream

        Ok(())
    }
}
