use anyhow::{Error, Result};
use async_trait::async_trait;
use futures::{
    future,
    stream::{self, repeat_with, select_with_strategy, BoxStream, PollNext},
    Stream, StreamExt,
};
use sea_orm::DatabaseConnection;
use std::{collections::HashSet, sync::Arc, time::Duration};
use tokio::{sync::RwLock, time::sleep};
use tracing::instrument;

use crate::{
    celestia, eigenda,
    settings::{DASettings, IndexerSettings},
};

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum Job {
    Celestia(celestia::job::CelestiaJob),
    EigenDA(eigenda::job::EigenDAJob),
}

#[async_trait]
pub trait DA {
    async fn process_job(&self, job: Job) -> Result<()>;
    async fn unprocessed_jobs(&self) -> Result<Vec<Job>>;
    async fn new_jobs(&self) -> Result<Vec<Job>>;
}

pub struct Indexer {
    da: Box<dyn DA + Send + Sync>,
    settings: IndexerSettings,

    failed_jobs: RwLock<HashSet<Job>>,
}

impl Indexer {
    pub async fn new(db: Arc<DatabaseConnection>, settings: IndexerSettings) -> Result<Self> {
        let da: Box<dyn DA + Send + Sync> = match settings.da.clone() {
            DASettings::Celestia(settings) => {
                Box::new(celestia::da::CelestiaDA::new(db.clone(), settings).await?)
            }
            DASettings::EigenDA(settings) => {
                Box::new(eigenda::da::EigenDA::new(db.clone(), settings).await?)
            }
        };
        Ok(Self {
            da,
            settings,
            failed_jobs: RwLock::new(HashSet::new()),
        })
    }

    #[instrument(name = "indexer", skip_all, level = "info")]
    pub async fn start(&self) -> anyhow::Result<()> {
        let mut stream = stream::SelectAll::<BoxStream<Job>>::new();
        stream.push(Box::pin(self.catch_up()));
        stream.push(Box::pin(self.retry_failed_jobs()));
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

    async fn process_job_with_retries(&self, job: &Job) {
        let mut backoff = vec![5, 20].into_iter().map(Duration::from_secs);
        while let Err(err) = &self.process_job(job).await {
            match backoff.next() {
                Some(delay) => {
                    tracing::warn!(error = ?err, job = ?job, ?delay, "failed to process job, retrying");
                    sleep(delay).await;
                }
                None => {
                    tracing::error!(error = ?err, job = ?job, "failed to process job, skipping for now, will retry later");
                    self.failed_jobs.write().await.insert(job.clone());
                    break;
                }
            };
        }
    }

    async fn process_job(&self, job: &Job) -> Result<()> {
        self.da.process_job(job.clone()).await
    }

    fn catch_up(&self) -> impl Stream<Item = Job> + '_ {
        repeat_with(move || async {
            sleep(self.settings.catchup_interval).await;
            self.da.unprocessed_jobs().await
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

    fn poll_for_new_jobs(&self) -> impl Stream<Item = Job> + '_ {
        repeat_with(|| async {
            sleep(self.settings.polling_interval).await;
            tracing::info!("polling for new jobs");
            self.da.new_jobs().await
        })
        .filter_map(|fut| async {
            fut.await
                .map_err(|err: Error| tracing::error!(error = ?err, "failed to poll for new jobs"))
                .ok()
        })
        .flat_map(stream::iter)
    }

    fn retry_failed_jobs(&self) -> impl Stream<Item = Job> + '_ {
        repeat_with(|| async {
            sleep(self.settings.retry_interval).await;
            // we can safely drain the failed blocks here
            // if the block fails again, it will be re-added
            // if the indexer will be restarted, catch_up will take care of it
            let iter = self.failed_jobs.write().await.drain().collect::<Vec<_>>();
            tracing::info!("retrying failed jobs: {:?}", iter);
            Ok(iter)
        })
        .filter_map(|fut| async {
            fut.await
                .map_err(|err: Error| tracing::error!(error = ?err, "failed to retry failed jobs"))
                .ok()
        })
        .flat_map(stream::iter)
    }
}
