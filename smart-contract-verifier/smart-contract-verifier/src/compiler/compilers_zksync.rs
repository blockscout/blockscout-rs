use crate::compiler::fetcher::FetchError;
use crate::{
    compiler::download_cache::DownloadCache, CompactVersion, DetailedVersion, Fetcher,
};
use async_trait::async_trait;
use foundry_compilers::error::SolcError;
use futures::TryFutureExt;
use std::marker::PhantomData;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{AcquireError, Semaphore};

#[derive(Debug, Error)]
pub enum ZkError {
    #[error("Zk compiler not found: {0}")]
    CompilerNotFound(String),
    #[error("Error while fetching compiler: {0:#}")]
    Fetch(#[from] FetchError),
    #[error("Internal error while compiling: {0}")]
    Internal(#[from] SolcError),
    #[error("Compilation error: {0:?}")]
    Compilation(Vec<String>),
    #[error("failed to acquire lock: {0}")]
    Acquire(#[from] AcquireError),
}

#[async_trait]
pub trait ZkSyncCompiler {
    type CompilerInput;

    async fn compile(
        zk_compiler_path: &Path,
        evm_compiler_path: &Path,
        input: &Self::CompilerInput,
    ) -> Result<serde_json::Value, SolcError>;
}

pub struct ZkSyncCompilers<ZkC> {
    evm_cache: DownloadCache<DetailedVersion>,
    evm_fetcher: Arc<dyn Fetcher<Version = DetailedVersion>>,
    zk_cache: DownloadCache<CompactVersion>,
    zk_fetcher: Arc<dyn Fetcher<Version = CompactVersion>>,
    threads_semaphore: Arc<Semaphore>,
    _phantom_data: PhantomData<ZkC>,
}

impl<ZkC: ZkSyncCompiler> ZkSyncCompilers<ZkC> {
    pub fn new(
        evm_fetcher: Arc<dyn Fetcher<Version = DetailedVersion>>,
        zk_fetcher: Arc<dyn Fetcher<Version = CompactVersion>>,
        threads_semaphore: Arc<Semaphore>,
    ) -> Self {
        Self {
            evm_cache: DownloadCache::default(),
            evm_fetcher,
            zk_cache: DownloadCache::default(),
            zk_fetcher,
            _phantom_data: Default::default(),
            threads_semaphore,
        }
    }

    pub async fn compile(
        &self,
        zk_compiler: &CompactVersion,
        evm_compiler: &DetailedVersion,
        input: &ZkC::CompilerInput,
    ) -> Result<serde_json::Value, ZkError> {
        let zk_path_future = self
            .zk_cache
            .get(self.zk_fetcher.as_ref(), zk_compiler)
            .map_err(|err| match err {
                FetchError::NotFound(version) => ZkError::CompilerNotFound(version),
                err => err.into(),
            });

        let evm_path_future = self
            .evm_cache
            .get(self.evm_fetcher.as_ref(), evm_compiler)
            .map_err(|err| match err {
                FetchError::NotFound(version) => ZkError::CompilerNotFound(version),
                err => err.into(),
            });

        let (zk_path_result, evm_path_result) = futures::join!(zk_path_future, evm_path_future);
        let zk_path = zk_path_result?;
        let evm_path = evm_path_result?;

        let _permit = self.threads_semaphore.acquire().await?;

        Ok(ZkC::compile(&zk_path, &evm_path, input).await?)
    }

    pub fn all_evm_versions_sorted_str(&self) -> Vec<String> {
        let mut versions = self.evm_fetcher.all_versions();
        // sort in descending order
        versions.sort_by(|x, y| x.cmp(y).reverse());
        versions.into_iter().map(|v| v.to_string()).collect()
    }

    pub fn all_zk_versions_sorted_str(&self) -> Vec<String> {
        let mut versions = self.zk_fetcher.all_versions();
        // sort in descending order
        versions.sort_by(|x, y| x.cmp(y).reverse());
        versions.into_iter().map(|v| v.to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        compiler::fetcher_list::ListFetcher, ZkSolcCompiler, DEFAULT_SOLIDITY_COMPILER_LIST,
        DEFAULT_ZKSOLC_COMPILER_LIST,
    };
    use std::env::temp_dir;

    async fn initialize() -> ZkSyncCompilers<ZkSolcCompiler> {
        let evm_url: url::Url = DEFAULT_SOLIDITY_COMPILER_LIST.try_into().unwrap();
        let zk_url = DEFAULT_ZKSOLC_COMPILER_LIST.try_into().unwrap();

        let evm_fetcher = ListFetcher::new(evm_url, temp_dir(), None, None)
            .await
            .expect("Fetch releases");
        let zk_evm_fetcher = ListFetcher::new(zk_url, temp_dir(), None, None)
            .await
            .expect("Fetch releases");

        let threads_semaphore = Arc::new(Semaphore::new(3));

        ZkSyncCompilers::new(
            Arc::new(evm_fetcher),
            Arc::new(zk_evm_fetcher),
            threads_semaphore,
        )
    }

    #[tokio::test]
    async fn can_fetch_and_list_evm_versions() {
        let compilers = initialize().await;
        let evm_versions = compilers.all_evm_versions_sorted_str();
        let zk_versions = compilers.all_zk_versions_sorted_str();

        assert!(
            evm_versions.contains(&"v0.8.25+commit.b61c2a91".to_string()),
            "{:#?}",
            evm_versions
        );
        assert!(
            zk_versions.contains(&"1.4.1".to_string()),
            "{:#?}",
            zk_versions
        );
    }
}
