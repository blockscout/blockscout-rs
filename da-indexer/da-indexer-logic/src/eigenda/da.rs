use anyhow::Result;
use async_trait::async_trait;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tokio::sync::RwLock;

use sea_orm::DatabaseConnection;

use crate::{
    common::{eth_provider::EthProvider, types::gap::Gap},
    eigenda::repository::{batches, blobs},
    indexer::{Job, DA},
};

use super::{client::Client, job::EigenDAJob, settings::IndexerSettings};

pub struct EigenDA {
    settings: IndexerSettings,

    db: Arc<DatabaseConnection>,
    client: Client,
    provider: EthProvider,

    last_known_block: AtomicU64,
    unprocessed_gaps: RwLock<Vec<Gap>>,
}

impl EigenDA {
    pub async fn new(db: Arc<DatabaseConnection>, settings: IndexerSettings) -> Result<Self> {
        let provider = EthProvider::new(&settings.rpc.url).await?;
        let client = Client::new(&settings.disperser, vec![5, 15, 30]).await?;
        let start_from = settings
            .start_height
            .unwrap_or(provider.get_block_number().await?);
        let gaps = batches::find_gaps(
            &db,
            settings.contract_creation_block as i64,
            start_from as i64,
        )
        .await?;
        Ok(Self {
            settings,
            db,
            client,
            provider,
            last_known_block: AtomicU64::new(start_from.saturating_sub(1)),
            unprocessed_gaps: RwLock::new(gaps),
        })
    }

    async fn jobs_from_block_range(
        &self,
        from: u64,
        to: u64,
        soft_limit: Option<u64>,
    ) -> Result<Vec<Job>> {
        let jobs = self
            .provider
            .get_logs(
                &self.settings.contract_address,
                "BatchConfirmed(bytes32,uint32)",
                from,
                to,
                self.settings.rpc.batch_size,
                soft_limit,
            )
            .await?
            .into_iter()
            .filter_map(|log| EigenDAJob::try_from(log).ok().map(Job::EigenDA))
            .collect();
        Ok(jobs)
    }
}

#[async_trait]
impl DA for EigenDA {
    async fn process_job(&self, job: Job) -> Result<()> {
        let job = EigenDAJob::from(job);
        tracing::info!(batch_id = job.batch_id, tx_hash = ?job.tx_hash, "processing batch");

        let mut blob_index = 0;
        let mut blobs = vec![];
        // it seems that there is no way to figure out the blobs count beforehand
        while let Some(blob) = self
            .client
            .retrieve_blob_with_retries(job.batch_id, &job.batch_header_hash, blob_index)
            .await?
        {
            blobs.push(blob);
            blob_index += 1;

            // blobs might be quite big, so we save them periodically
            // to save ram and to avoid huge db transactions
            if blobs.len() == self.settings.save_batch_size as usize {
                blobs::upsert_many(
                    self.db.as_ref(),
                    blob_index as i32 - blobs.len() as i32,
                    &job.batch_header_hash,
                    blobs,
                )
                .await?;
                blobs = vec![];
            }
        }

        if !blobs.is_empty() {
            blobs::upsert_many(
                self.db.as_ref(),
                blob_index as i32 - blobs.len() as i32,
                &job.batch_header_hash,
                blobs,
            )
            .await?;
        }

        let blobs_len = blob_index;
        tracing::info!(
            batch_id = job.batch_id,
            "retrieved {} blobs",
            blobs_len
        );

        if blobs_len == 0
            && self.last_known_block.load(Ordering::Relaxed) - job.block_number
                < self.settings.pruning_block_threshold
        {
            return Err(anyhow::anyhow!("no blobs retrieved for recent batch"));
        }

        batches::upsert(
            self.db.as_ref(),
            &job.batch_header_hash,
            job.batch_id as i64,
            blobs_len as i32,
            job.tx_hash.as_bytes(),
            job.block_number as i64,
        )
        .await?;

        Ok(())
    }

    async fn new_jobs(&self) -> Result<Vec<Job>> {
        let from = self.last_known_block.load(Ordering::Acquire) + 1;
        let to = self.provider.get_block_number().await?;

        let jobs = self.jobs_from_block_range(from, to, None).await?;
        self.last_known_block.store(to, Ordering::Release);
        Ok(jobs)
    }

    /// Returns the earliest unprocessed batch or multiple batches
    /// if there are many in the same block
    async fn unprocessed_jobs(&self) -> Result<Vec<Job>> {
        let mut unprocessed_gaps = self.unprocessed_gaps.write().await;
        tracing::info!("catching up gaps: {:?}", unprocessed_gaps);

        let mut jobs = vec![];
        let mut new_gaps = vec![];

        for gap in unprocessed_gaps.iter() {
            match jobs.is_empty() {
                true => {
                    let jobs_in_range = self
                        .jobs_from_block_range(gap.start as u64, gap.end as u64, Some(1))
                        .await?;

                    if !jobs_in_range.is_empty() {
                        let block_number = EigenDAJob::block_number(jobs_in_range.first().unwrap());
                        // in case there are multiple batches in the same block
                        jobs.extend(
                            jobs_in_range
                                .into_iter()
                                .take_while(|job| EigenDAJob::block_number(job) == block_number),
                        );

                        if block_number < gap.end as u64 {
                            new_gaps.push(Gap {
                                start: block_number as i64 + 1,
                                end: gap.end,
                            });
                        }
                    }
                }
                false => new_gaps.push(gap.clone()),
            }
        }
        *unprocessed_gaps = new_gaps;

        Ok(jobs)
    }
}
