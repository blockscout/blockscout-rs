use crate::{compiler::download_cache::DownloadCache, Fetcher, Version};
use std::{path::PathBuf, sync::Arc};

pub struct ZksyncCompilers {
    evm_cache: DownloadCache,
    evm_fetcher: Arc<dyn Fetcher>,
    zk_cache: DownloadCache,
    zk_fetcher: Arc<dyn Fetcher>,
}

impl ZksyncCompilers {
    pub fn new(evm_fetcher: Arc<dyn Fetcher>, zk_fetcher: Arc<dyn Fetcher>) -> Self {
        Self {
            evm_cache: DownloadCache::default(),
            evm_fetcher,
            zk_cache: DownloadCache::default(),
            zk_fetcher,
        }
    }

    pub fn all_evm_versions(&self) -> Vec<Version> {
        self.evm_fetcher.all_versions()
    }

    pub fn all_evm_versions_sorted_str(&self) -> Vec<String> {
        let mut versions = self.all_evm_versions();
        // sort in descending order
        versions.sort_by(|x, y| x.cmp(y).reverse());
        versions.into_iter().map(|v| v.to_string()).collect()
    }

    pub fn all_zk_versions(&self) -> Vec<Version> {
        self.zk_fetcher.all_versions()
    }

    pub fn all_zk_versions_sorted_str(&self) -> Vec<String> {
        let mut versions = self.all_zk_versions();
        // sort in descending order
        versions.sort_by(|x, y| x.cmp(y).reverse());
        versions.into_iter().map(|v| v.to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ListFetcher, DEFAULT_SOLIDITY_COMPILER_LIST};
    use std::{env::temp_dir, str::FromStr};

    async fn initialize() -> ZksyncCompilers {
        let evm_url = DEFAULT_SOLIDITY_COMPILER_LIST
            .try_into()
            .expect("Getting url");
        let evm_fetcher = ListFetcher::new(evm_url, temp_dir(), None, None)
            .await
            .expect("Fetch releases");

        let fetcher = Arc::new(evm_fetcher);
        ZksyncCompilers::new(fetcher.clone(), fetcher.clone())
    }

    #[tokio::test]
    async fn can_fetch_and_list_evm_versions() {
        let compilers = initialize().await;
        let evm_versions = compilers.all_evm_versions_sorted_str();

        assert!(evm_versions.contains(&"v0.8.25+commit.b61c2a91".to_string()));
    }
}
