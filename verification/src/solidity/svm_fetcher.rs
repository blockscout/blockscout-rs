use crate::compiler::{fetcher::Fetcher, version::CompilerVersion};
use async_trait::async_trait;
use ethers_solc::Solc;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Default)]
pub struct SvmFetcher {}

#[derive(Error, Debug)]
pub enum FetchError {
    #[error("svm returned error {0}")]
    Svm(svm::SolcVmError),
    #[error("nightly versions aren't supported in svm")]
    NightlyNotSupported,
}

#[async_trait]
impl Fetcher for SvmFetcher {
    type Error = FetchError;
    async fn fetch(&self, ver: &CompilerVersion) -> Result<PathBuf, Self::Error> {
        match ver {
            CompilerVersion::Release(ver) => Solc::install(&ver.version)
                .await
                .map(|x| x.solc)
                .map_err(FetchError::Svm),
            CompilerVersion::Nightly(_) => Err(FetchError::NightlyNotSupported),
        }
    }
}
