use super::{job::CelestiaJob, parser, repository::blocks, settings::IndexerSettings};
use crate::{
    celestia::{client, repository::blobs},
    indexer::{Job, DA},
    s3_storage::S3Storage,
};
use anyhow::Result;
use async_trait::async_trait;
use celestia_rpc::{Client, HeaderClient, ShareClient};
use celestia_types::{Blob, ExtendedHeader};
use sea_orm::{DatabaseConnection, TransactionTrait};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

pub struct CelestiaDA {
    settings: IndexerSettings,

    client: Client,
    db: Arc<DatabaseConnection>,
    s3_storage: Option<S3Storage>,

    last_known_height: AtomicU64,
    catch_up_completed: AtomicBool,
}

impl CelestiaDA {
    pub async fn new(
        db: Arc<DatabaseConnection>,
        s3_storage: Option<S3Storage>,
        settings: IndexerSettings,
    ) -> Result<Self> {
        let client = client::new_celestia_client(
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
            settings,
            client,
            db,
            s3_storage,
            last_known_height: AtomicU64::new(start_from.saturating_sub(1)),
            catch_up_completed: AtomicBool::new(false),
        })
    }

    async fn get_blobs_by_height(&self, height: u64) -> Result<(ExtendedHeader, Vec<Blob>)> {
        let header = self.client.header_get_by_height(height).await?;

        let mut blobs = vec![];
        if parser::maybe_contains_blobs(&header.dah) {
            let eds = self.client.share_get_eds(&header).await?;
            blobs = parser::parse_eds(&eds, header.header.version.app)?;
        }

        Ok((header, blobs))
    }
}

#[async_trait]
impl DA for CelestiaDA {
    async fn process_job(&self, job: Job) -> anyhow::Result<()> {
        let job: CelestiaJob = job.into();
        let (header, blobs) = self.get_blobs_by_height(job.height).await?;

        let txn = self.db.begin().await?;

        let blobs_count = blobs.len() as u32;

        blocks::upsert(
            &txn,
            job.height,
            header.hash().as_bytes(),
            blobs_count,
            header.header.time.unix_timestamp(),
        )
        .await?;

        if !blobs.is_empty() {
            // blobs might be quite big, so we save them periodically
            // to save ram and to avoid huge db insertions
            for chunk in blobs.chunks(self.settings.save_batch_size as usize) {
                blobs::upsert_many(&txn, self.s3_storage.as_ref(), job.height, chunk.to_vec())
                    .await?;
            }
            tracing::debug!(height = job.height, blobs_count, "saved blobs to db");
        }

        txn.commit().await?;

        // this is not accurate, just to indicate progress
        if job.height % 1000 == 0 {
            tracing::info!(height = job.height, "processed height");
        }

        Ok(())
    }

    async fn new_jobs(&self) -> anyhow::Result<Vec<Job>> {
        let height = self.client.header_local_head().await?.header.height.value();
        tracing::info!(height, "latest block");

        if height <= self.last_known_height.load(Ordering::Acquire) {
            tracing::info!("latest block is below last known height, skipping...");
            return Ok(vec![]);
        }

        let from = self.last_known_height.swap(height, Ordering::AcqRel) + 1;
        Ok((from..=height)
            .map(|height| Job::Celestia(CelestiaJob { height }))
            .collect())
    }

    async fn unprocessed_jobs(&self) -> anyhow::Result<Vec<Job>> {
        if self.catch_up_completed.load(Ordering::Acquire) {
            return Ok(vec![]);
        }

        // TODO: do we need genesis block metadata?
        if !blocks::exists(&self.db, 0).await? {
            blocks::upsert(self.db.as_ref(), 0, &[], 0, 0).await?;
        }

        let last_known_height = self.last_known_height.load(Ordering::Acquire);
        let gaps = blocks::find_gaps(&self.db, last_known_height).await?;

        tracing::info!("catch up gaps: [{:?}]", gaps);
        self.catch_up_completed.store(true, Ordering::Release);

        Ok(gaps
            .into_iter()
            .flat_map(|gap| {
                (gap.start..=gap.end).map(|height| {
                    Job::Celestia(CelestiaJob {
                        height: height as u64,
                    })
                })
            })
            .rev()
            .collect())
    }
}
