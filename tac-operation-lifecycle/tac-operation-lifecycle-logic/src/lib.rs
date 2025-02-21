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
use tracing::instrument;
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
    #[instrument(name = "indexer", skip_all, level = "info")]
    pub async fn save_intervals(&self) -> anyhow::Result<()> {
        let current_timestamp = chrono::Utc::now().timestamp();
        let watermark = self.watermark.load(std::sync::atomic::Ordering::Acquire);
        let intervals = (watermark as i64..current_timestamp as i64)
            .map(|timestamp| interval::ActiveModel {
                start: ActiveValue::Set(timestamp),
                end: ActiveValue::Set(timestamp + self.settings.catchup_interval.as_secs() as i64),
                timestamp: ActiveValue::Set(chrono::Utc::now().timestamp()),
                id: ActiveValue::NotSet,
            })
            .collect::<Vec<_>>();

        //intitiate a transaction
        let tx = self.db.begin().await?;
        //save intervals to db
        interval::Entity::insert_many(intervals)
            .exec_with_returning(&tx)
            .await?;
        //commit the transaction
        tx.commit().await?;

        Ok(())
    }

    pub async fn process_job_with_retries(&self, _job: &Job) -> () {
        ()
    }
    #[instrument(name = "indexer", skip_all, level = "info")]
    pub async fn start(&self) -> anyhow::Result<()> {
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
            println!("polling for new jobs");
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
            println!("catching up");
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
