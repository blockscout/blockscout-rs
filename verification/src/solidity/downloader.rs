use std::path::PathBuf;

use crate::download_cache::Fetcher;
use anyhow::{Context, Result};
use async_trait::async_trait;
use ethers_solc::Solc;
use semver::Version;

#[derive(Default)]
pub struct SolcDownloader {}

#[async_trait]
impl Fetcher for SolcDownloader {
    async fn fetch(&self, ver: &Version) -> Result<PathBuf> {
        Solc::install(ver)
            .await
            .map(|x| x.solc)
            .context("Failed to download solc")
    }
}
