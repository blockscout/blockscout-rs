use crate::types::Mismatch;

use super::version::Version;
use async_trait::async_trait;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FetchError {
    #[error("version {0} not found")]
    NotFound(Version),
    #[error("couldn't fetch the file: {0}")]
    Fetch(anyhow::Error),
    #[error("hashsum of fecthed file mismatch: {0}")]
    HashMismatch(Mismatch<String>),
    #[error("couldn't create file: {0}")]
    File(std::io::Error),
    #[error("tokio sheduling error: {0}")]
    Schedule(tokio::task::JoinError),
}

#[async_trait]
pub trait Fetcher: Send + Sync {
    async fn fetch(&self, ver: &Version) -> Result<PathBuf, FetchError>;
    fn all_versions(&self) -> Vec<Version>;
}
