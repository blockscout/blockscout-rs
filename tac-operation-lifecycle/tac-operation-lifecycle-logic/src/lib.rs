use std::{
    cmp::{max, min},
    sync::{atomic::AtomicU64, Arc}, time::Duration,
};

use anyhow::Error;
use client::Client;

pub mod client;
pub mod settings;

use futures::{
    future,
    stream::{self, repeat_with, select_with_strategy, BoxStream, PollNext},
    Stream, StreamExt,
};
use sea_orm::ColumnTrait;
use sea_orm::{ActiveValue, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait};
use settings::IndexerSettings;
use tac_operation_lifecycle_entity::{interval, watermark};
use tokio::time::sleep;
pub struct Indexer {
    client: Client,
    settings: IndexerSettings,
    watermark: AtomicU64,
    db: Arc<DatabaseConnection>,
}
use chrono;
use tracing::{instrument, Instrument};
use sea_orm::ActiveModelTrait;
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct Job {
    pub start: u64,
    pub end: u64,
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
                status:sea_orm::ActiveValue::Set(0 as i16)
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
    

    pub async fn process_job_with_retries(&self, _job: &Job) -> () {
        tracing::info!("processing job");
        ()
    }

    pub fn poll_for_new_jobs(&self) -> BoxStream<'_, Job> {
        use sea_orm::{Statement, FromQueryResult};
        
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

                // Find and lock a single pending interval with highest priority
                let stmt = Statement::from_sql_and_values(
                    sea_orm::DatabaseBackend::Postgres,
                    r#"
                    SELECT id, start, "end", timestamp, status
                    FROM interval
                    WHERE status = 0
                    ORDER BY start DESC
                    LIMIT 1
                    FOR UPDATE SKIP LOCKED
                    "#,
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
                    RETURNING id, start, "end", timestamp, status
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

                            yield Job {
                                start: updated.start as u64,
                                end: updated.end as u64,
                            };
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

    #[instrument(name = "indexer", skip_all, level = "info")]
    pub async fn start(&self) -> anyhow::Result<()> {
        tracing::warn!("starting indexer");
        self.save_intervals().await?;
        
        let mut stream = stream::SelectAll::<BoxStream<'_, Job>>::new();
        // stream.push(Box::pin(self.catch_up()));
        stream.push(self.poll_for_new_jobs());

        stream
            .for_each_concurrent(Some(self.settings.concurrency as usize), |job| async move {
                self.process_job_with_retries(&job).await
            })
            .await;

        Ok(())
    }

    fn catch_up(&self) -> impl Stream<Item = Job> + '_ {
        repeat_with(move || async {
            sleep(self.settings.polling_interval).await;
            tracing::info!("catching up");
            Ok(vec![Job { start: 1, end: 100 }])
        })
        .filter_map(|fut| async {
            fut.await
            .map_err(
                |err: Error| tracing::error!(error = ?err, "failed to retrieve unprocessed jobs"),
            )
            .ok()
        })
        .take_while(|jobs| future::ready(!jobs.is_empty()))
        .flat_map(stream::iter)
    }
}
