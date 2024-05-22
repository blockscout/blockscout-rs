use super::{generic_fetcher};
use crate::{compiler::download_cache::DownloadCache, Fetcher, Version};
use std::{sync::Arc};

pub struct ZksyncCompilers {
    evm_cache: DownloadCache,
    evm_fetcher: Arc<dyn generic_fetcher::Fetcher<Version = Version>>,
    zk_cache: DownloadCache,
    zk_fetcher: Arc<dyn generic_fetcher::Fetcher<Version = semver::Version>>,
}

impl ZksyncCompilers {
    pub fn new(
        evm_fetcher: Arc<dyn generic_fetcher::Fetcher<Version=Version>>,
        zk_fetcher: Arc<dyn generic_fetcher::Fetcher<Version = semver::Version>>,
    ) -> Self {
        Self {
            evm_cache: DownloadCache::default(),
            evm_fetcher,
            zk_cache: DownloadCache::default(),
            zk_fetcher,
        }
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
    use crate::{compiler::generic_list_fetcher::ListFetcher, DEFAULT_SOLIDITY_COMPILER_LIST, DEFAULT_ZKSOLC_COMPILER_LIST};
    use std::{env::temp_dir};

    async fn initialize() -> ZksyncCompilers {
        let evm_url: url::Url = DEFAULT_SOLIDITY_COMPILER_LIST
            .try_into()
            .unwrap();
        let zk_url = DEFAULT_ZKSOLC_COMPILER_LIST
            .try_into()
            .unwrap();

        let evm_fetcher = ListFetcher::new(evm_url, temp_dir(), None, None)
            .await
            .expect("Fetch releases");
        let zk_evm_fetcher = ListFetcher::new(zk_url, temp_dir(), None, None)
            .await
            .expect("Fetch releases");

        ZksyncCompilers::new(Arc::new(evm_fetcher), Arc::new(zk_evm_fetcher))
    }

    #[tokio::test]
    async fn can_fetch_and_list_evm_versions() {
        let compilers = initialize().await;
        let evm_versions = compilers.all_evm_versions_sorted_str();
        let zk_versions = compilers.all_zk_versions_sorted_str();

        assert!(evm_versions.contains(&"v0.8.25+commit.b61c2a91".to_string()), "{:#?}", evm_versions);
        assert!(zk_versions.contains(&"1.4.1".to_string()), "{:#?}", zk_versions);
    }
}
