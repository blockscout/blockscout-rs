use crate::download_cache::Fetcher;
use async_trait::async_trait;
use ethers_solc::Solc;
use semver::Version;
use std::path::PathBuf;

#[derive(Default)]
pub struct SvmFetcher {}

#[async_trait]
impl Fetcher for SvmFetcher {
    type Error = svm::SolcVmError;
    async fn fetch(&self, ver: &Version) -> Result<PathBuf, Self::Error> {
        Solc::install(ver).await.map(|x| x.solc)
    }
}
