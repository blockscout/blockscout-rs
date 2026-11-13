// SPDX-License-Identifier: LicenseRef-Blockscout

use super::fetcher::{FetchError, Fetcher, Version};
use async_trait::async_trait;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Serves the compilers of several fetchers as one list. A version is taken from the first
/// fetcher that lists it.
pub struct CombinedFetcher<Ver> {
    fetchers: Vec<Arc<dyn Fetcher<Version = Ver>>>,
}

impl<Ver: Version> CombinedFetcher<Ver> {
    pub fn new(fetchers: Vec<Arc<dyn Fetcher<Version = Ver>>>) -> Self {
        Self { fetchers }
    }

    fn owner(&self, ver: &Ver) -> Result<&dyn Fetcher<Version = Ver>, FetchError> {
        self.fetchers
            .iter()
            .find(|fetcher| fetcher.all_versions().contains(ver))
            .map(|fetcher| fetcher.as_ref())
            .ok_or_else(|| FetchError::NotFound(ver.to_string()))
    }
}

#[async_trait]
impl<Ver: Version> Fetcher for CombinedFetcher<Ver> {
    type Version = Ver;

    async fn fetch(&self, ver: &Self::Version) -> Result<PathBuf, FetchError> {
        self.owner(ver)?.fetch(ver).await
    }

    async fn validate_file(&self, ver: &Self::Version, path: &Path) -> Result<(), FetchError> {
        self.owner(ver)?.validate_file(ver, path).await
    }

    fn all_versions(&self) -> Vec<Self::Version> {
        self.fetchers
            .iter()
            .flat_map(|fetcher| fetcher.all_versions())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    struct StaticFetcher {
        name: &'static str,
        versions: Vec<semver::Version>,
    }

    #[async_trait]
    impl Fetcher for StaticFetcher {
        type Version = semver::Version;

        async fn fetch(&self, ver: &Self::Version) -> Result<PathBuf, FetchError> {
            Ok(PathBuf::from(format!("{}/{ver}", self.name)))
        }

        async fn validate_file(&self, _ver: &Self::Version, path: &Path) -> Result<(), FetchError> {
            match path.starts_with(self.name) {
                true => Ok(()),
                false => Err(FetchError::Validation(anyhow::anyhow!("not ours"))),
            }
        }

        fn all_versions(&self) -> Vec<Self::Version> {
            self.versions.clone()
        }
    }

    fn version(s: &str) -> semver::Version {
        semver::Version::parse(s).unwrap()
    }

    fn combined() -> CombinedFetcher<semver::Version> {
        CombinedFetcher::new(vec![
            Arc::new(StaticFetcher {
                name: "first",
                versions: vec![version("1.0.0"), version("1.1.0")],
            }),
            Arc::new(StaticFetcher {
                name: "second",
                versions: vec![version("1.1.0"), version("2.0.0")],
            }),
        ])
    }

    #[tokio::test]
    async fn routes_a_version_to_the_first_fetcher_listing_it() {
        let fetcher = combined();
        for (ver, expected) in [
            ("1.0.0", "first/1.0.0"),
            ("1.1.0", "first/1.1.0"),
            ("2.0.0", "second/2.0.0"),
        ] {
            let path = fetcher.fetch(&version(ver)).await.unwrap();
            assert_eq!(path, PathBuf::from(expected));
            fetcher.validate_file(&version(ver), &path).await.unwrap();
        }

        let error = fetcher.fetch(&version("3.0.0")).await.unwrap_err();
        assert!(matches!(error, FetchError::NotFound(_)), "{error:?}");
        let error = fetcher
            .validate_file(&version("3.0.0"), Path::new("first/3.0.0"))
            .await
            .unwrap_err();
        assert!(matches!(error, FetchError::NotFound(_)), "{error:?}");
    }

    #[test]
    fn lists_the_versions_of_all_fetchers() {
        assert_eq!(
            combined().all_versions(),
            ["1.0.0", "1.1.0", "1.1.0", "2.0.0"].map(version)
        );
    }
}
