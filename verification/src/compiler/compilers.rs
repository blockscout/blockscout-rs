use crate::compiler::{CompilerVersion, DownloadCache, Fetcher};
use anyhow::anyhow;
use ethers_solc::{error::SolcError, CompilerInput, CompilerOutput, Solc};
use std::fmt::{Debug, Display};
use thiserror::Error;

use super::fetcher::VersionList;

#[derive(Debug, Error)]
pub enum CompilersError {
    #[error("Error while fetching compiler: {0:#}")]
    Fetch(anyhow::Error),
    #[error("Compilation error: {0}")]
    Compilation(#[from] SolcError),
}

pub struct Compilers<T> {
    cache: DownloadCache,
    fetcher: T,
}

impl<T: Fetcher> Compilers<T> {
    pub fn new(fetcher: T) -> Self {
        Self {
            cache: DownloadCache::new(),
            fetcher,
        }
    }

    pub async fn compile(
        &self,
        compiler_version: &CompilerVersion,
        input: &CompilerInput,
    ) -> Result<CompilerOutput, CompilersError>
    where
        <T as Fetcher>::Error: Debug + Display,
    {
        let solc_path = self
            .cache
            .get(&self.fetcher, compiler_version)
            .await
            .map_err(|err| CompilersError::Fetch(anyhow!(err)))?;
        let solc = Solc::from(solc_path);
        let output = solc.compile(&input)?;

        Ok(output)
    }
}

impl<T: VersionList> Compilers<T> {
    pub fn all_versions(&self) -> Vec<&CompilerVersion> {
        self.fetcher.all_versions()
    }
}
