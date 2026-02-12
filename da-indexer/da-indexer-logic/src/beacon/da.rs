use super::settings::IndexerSettings;
use crate::{
    indexer::{Job, DA},
    s3_storage::S3Storage,
};
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub struct BeaconDA {
    settings: IndexerSettings,

    db: Arc<DatabaseConnection>,
    s3_storage: S3Storage,
}

impl BeaconDA {
    pub async fn new(
        db: Arc<DatabaseConnection>,
        s3_storage: S3Storage,
        settings: IndexerSettings,
    ) -> anyhow::Result<Self> {
        todo!()
    }
}

#[async_trait]
impl DA for BeaconDA {
    async fn process_job(&self, job: Job) -> anyhow::Result<()> {
        todo!()
    }

    async fn unprocessed_jobs(&self) -> anyhow::Result<Vec<Job>> {
        todo!()
    }

    async fn new_jobs(&self) -> anyhow::Result<Vec<Job>> {
        // let new_head =
        todo!()
    }
}
