use anyhow::{Error, Result};
use celestia_rpc::{prelude::*, Client};
use celestia_types::{blob::Blob, ExtendedHeader};
use futures::{
    stream,
    stream::{repeat_with, BoxStream},
    Stream, StreamExt,
};
use sea_orm::DatabaseConnection;
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{sync::RwLock, time::sleep};
use tracing::instrument;

use crate::celestia::rpc_client;

use super::{
    parser,
    repository::{blobs, blocks},
    settings::IndexerSettings,
};

#[derive(Debug)]
struct Job {
    height: u64,
}

pub struct Indexer {
    client: Client,
    db: Arc<DatabaseConnection>,
    settings: IndexerSettings,

    last_known_height: AtomicU64,
    failed_blocks: RwLock<HashSet<u64>>,
}

impl Indexer {
    pub async fn new(db: Arc<DatabaseConnection>, settings: IndexerSettings) -> Result<Self> {
        let client = rpc_client::new_celestia_client(
            &settings.rpc.url,
            settings.rpc.auth_token.as_deref(),
            settings.rpc.max_request_size,
            settings.rpc.max_response_size,
        )
        .await?;

        let mut start_from = settings
            .start_height
            .unwrap_or(client.header_local_head().await?.header.height.value());

        // skip genesis block, it can't be fetched by usual means
        if start_from == 0 {
            start_from = 1;
        }

        tracing::info!(start_from, "indexer initialized");

        Ok(Self {
            client,
            db,
            settings,
            last_known_height: AtomicU64::new(start_from.saturating_sub(1)),
            failed_blocks: RwLock::new(HashSet::new()),
        })
    }

    #[instrument(name = "celestia_indexer", skip_all)]
    pub async fn start(&self) -> anyhow::Result<()> {
        let mut stream = stream::SelectAll::<BoxStream<Job>>::new();
        stream.push(Box::pin(self.catch_up().await?));
        stream.push(Box::pin(self.poll_for_new_blocks()));
        stream.push(Box::pin(self.retry_failed_blocks()));

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
                None => {
                    tracing::warn!(error = ?err, height = job.height, "failed to process job, skipping for now, will retry later");
                    self.failed_blocks.write().await.insert(job.height);
                    break;
                }
                Some(delay) => {
                    tracing::error!(error = ?err, height = job.height, ?delay, "failed to process job, retrying");
                    sleep(delay).await;
                }
            };
        }
    }

    async fn process_job(&self, job: &Job) -> Result<()> {
        let (header, blobs) = self.get_blobs_by_height(job.height).await?;

        let blobs_count = blobs.len() as u32;
        if !blobs.is_empty() {
            blobs::upsert_many(&self.db, job.height, blobs).await?;
            tracing::debug!(height = job.height, blobs_count, "saved blobs to db");
        }

        blocks::upsert(
            &self.db,
            job.height,
            header.hash().as_bytes(),
            blobs_count,
            header.header.time.unix_timestamp(),
        )
        .await?;

        // this is not accurate, just to indicate progress
        if job.height % 1000 == 0 {
            tracing::info!(height = job.height, "processed height");
        }

        Ok(())
    }

    async fn get_blobs_by_height(&self, height: u64) -> Result<(ExtendedHeader, Vec<Blob>)> {
        let header = self.client.header_get_by_height(height).await?;
        let mut blobs = vec![];

        if parser::maybe_contains_blobs(&header.dah) {
            let eds = self.client.share_get_eds(&header).await?;
            blobs = parser::parse_eds(&eds, header.dah.square_len())?;
        }

        Ok((header, blobs))
    }

    async fn catch_up(&self) -> Result<impl Stream<Item = Job> + '_> {
        // TODO: do we need genesis block metadata?
        if !blocks::exists(&self.db, 0).await? {
            blocks::upsert(&self.db, 0, &[], 0, 0).await?;
        }

        let last_known_height = self.last_known_height.load(Ordering::SeqCst);
        let gaps = blocks::find_gaps(&self.db, last_known_height).await?;

        tracing::info!("catch up gaps: {:?}", gaps);

        Ok(stream::iter(
            gaps.into_iter()
                .flat_map(|gap| {
                    (gap.gap_start..=gap.gap_end).map(|height| Job {
                        height: height as u64,
                    })
                })
                .rev(),
        ))
    }

    fn poll_for_new_blocks(&self) -> impl Stream<Item = Job> + '_ {
        repeat_with(|| async {
            sleep(self.settings.polling_interval).await;

            let height = self.client.header_local_head().await?.header.height.value();
            tracing::info!(height, "latest block");

            let from = self.last_known_height.swap(height, Ordering::SeqCst) + 1;
            Ok((from..=height).map(|height| Job { height }))
        })
        .filter_map(|fut| async {
            fut.await
                .map_err(
                    |err: Error| tracing::error!(error = ?err, "failed to poll for new blocks"),
                )
                .ok()
        })
        .flat_map(stream::iter)
    }

    fn retry_failed_blocks(&self) -> impl Stream<Item = Job> + '_ {
        repeat_with(|| async {
            sleep(self.settings.retry_interval).await;
            // we can safely drain the failed blocks here
            // if the block fails again, it will be re-added
            // if the indexer will be restarted, catch_up will take care of it
            let iter = self
                .failed_blocks
                .write()
                .await
                .drain()
                .map(|height| Job { height })
                .collect::<Vec<_>>();
            tracing::info!("retrying failed blocks: {:?}", iter);
            Ok(iter)
        })
        .filter_map(|fut| async {
            fut.await
                .map_err(
                    |err: Error| tracing::error!(error = ?err, "failed to retry failed blocks"),
                )
                .ok()
        })
        .flat_map(stream::iter)
    }
}
