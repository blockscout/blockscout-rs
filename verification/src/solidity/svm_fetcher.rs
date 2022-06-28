use crate::compiler::{CompilerVersion, Fetcher};
use async_trait::async_trait;
use ethers_solc::{error::SolcError, Solc};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Default)]
pub struct SvmFetcher {}

#[derive(Error, Debug)]
pub enum FetchError {
    #[error("svm returned error {0}")]
    Svm(#[from] SolcError),
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
                .map_err(|err| FetchError::Svm(err.into())),
            CompilerVersion::Nightly(_) => Err(FetchError::NightlyNotSupported),
        }
    }
}
