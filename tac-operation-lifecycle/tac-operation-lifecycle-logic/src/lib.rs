use std::{
    cmp::max,
    sync::{atomic::AtomicU64, Arc},
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
        let current_timestamp = chrono::Utc::now().timestamp() as u64;
        let watermark = self.watermark.load(std::sync::atomic::Ordering::Acquire);
        let batch_size = 1000; // Adjust this based on your needs and database performance
        let catch_up_interval = self.settings.catchup_interval.as_secs() as u64;
        let intervals: Vec<interval::ActiveModel> = (watermark as u64..current_timestamp)
            .map(|timestamp| interval::ActiveModel {
                start: ActiveValue::Set(timestamp as i64),
                end: ActiveValue::Set((timestamp + catch_up_interval) as i64),
                timestamp: ActiveValue::Set(chrono::Utc::now().timestamp()),
                id: ActiveValue::NotSet,
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
                    let last_interval_end = watermark + (batch_size as u64) * catch_up_interval;
                    
                    // Find existing watermark or create new one
                    let existing_watermark = watermark::Entity::find()
                        .one(&tx)
                        .await?;

                    let watermark_model = match existing_watermark {
                        Some(w) => watermark::ActiveModel {
                            id: ActiveValue::Unchanged(w.id),
                            timestamp: ActiveValue::Set(last_interval_end as i64),
                        },
                        None => watermark::ActiveModel {
                            id: ActiveValue::NotSet,
                            timestamp: ActiveValue::Set(last_interval_end as i64),
                        },
                    };

                    // Save the updated watermark
                    watermark_model.save(&tx).await?;
                    
                    // Update the in-memory watermark
                    self.watermark.store(last_interval_end, std::sync::atomic::Ordering::Release);
                    
                    tx.commit().await?;
                    tracing::info!(
                        "Successfully saved batch of {} intervals and updated watermark to {}",
                        chunk.len(),
                        last_interval_end
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
    #[instrument(name = "get_next_interval", skip_all, level = "info")]
    pub async fn get_next_interval(&self) -> anyhow::Result<()> {
        let current_timestamp = chrono::Utc::now().timestamp();
        let watermark = self.watermark.load(std::sync::atomic::Ordering::Acquire);
        let next_interval = interval::ActiveModel {
            start: ActiveValue::Set(watermark as i64),
            end: ActiveValue::Set(watermark as i64 + self.settings.catchup_interval.as_secs() as i64),
            timestamp: ActiveValue::Set(chrono::Utc::now().timestamp()),
            id: ActiveValue::NotSet,
        };
        // let intervals = (watermark as i64..current_timestamp as i64)
        //     .map(|timestamp| interval::ActiveModel {
        //         start: ActiveValue::Set(timestamp),
        //         end: ActiveValue::Set(timestamp + self.settings.catchup_interval.as_secs() as i64),
        //         timestamp: ActiveValue::Set(chrono::Utc::now().timestamp()),
        //         id: ActiveValue::NotSet,
        //     })
        //     .collect::<Vec<_>>();

        // tracing::info!("saving {} intervals", intervals.len());
        //intitiate a transaction
        // let tx = self.db.begin().await?;
        // //save intervals to db
        // interval::Entity::insert_many(intervals)
        //     .exec_with_returning(&tx)
        //     .await?;
        // //commit the transaction
        // tx.commit().await?;

        Ok(())
    }

    pub async fn process_job_with_retries(&self, _job: &Job) -> () {
        tracing::info!("processing job");
        ()
    }
    #[instrument(name = "indexer", skip_all, level = "info")]
    pub async fn start(&self) -> anyhow::Result<()> {
        tracing::warn!("starting indexer");
        self.save_intervals().await?;
        let mut stream = stream::SelectAll::<BoxStream<Job>>::new();
        stream.push(Box::pin(self.catch_up()));
        // stream.push(Box::pin(self.retry_failed_jobs()));
        let stream =
            select_with_strategy(Box::pin(self.poll_for_new_jobs()), stream, |_: &mut ()| {
                PollNext::Left
            });

        stream
            .for_each_concurrent(Some(self.settings.concurrency as usize), |job| async move {
                self.process_job_with_retries(&job).await
            })
            .await;

        Ok(())
    }

    fn poll_for_new_jobs(&self) -> impl Stream<Item = Job> + '_ {
        repeat_with(move || async {
            sleep(self.settings.polling_interval).await;
            tracing::info!("polling for new jobs");
            Ok(vec![Job {
                start: 1,
                end: 100,
            }])
        }).filter_map(|fut| async {
            fut.await
                .map_err(
                    |err: Error| tracing::error!(error = ?err, "failed to retrieve unprocessed jobs"),
                )
                .ok()
        }).take_while(|jobs| future::ready(!jobs.is_empty()))
        .flat_map(stream::iter)
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
