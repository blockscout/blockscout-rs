use crate::compiler::{
    fetcher::Fetcher,
    version::{CompilerVersion, ParseError},
};
use async_trait::async_trait;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::ErrorKind,
    os::unix::prelude::OpenOptionsExt,
    path::{Path, PathBuf},
    str::FromStr,
};
use thiserror::Error;
use url::Url;

#[derive(Default)]
pub struct GithubFetcher {
    releases: HashMap<CompilerVersion, url::Url>,
    folder: PathBuf,
}

#[derive(Error, Debug)]
pub enum ListError {
    #[error("fetching github returned error: {0}")]
    Github(octocrab::Error),
    #[error("error parsing tag name into version: {0}")]
    TagName(ParseError),
}

impl GithubFetcher {
    async fn list_releases(
        owner: &str,
        repo: &str,
    ) -> Result<HashMap<CompilerVersion, Url>, ListError> {
        let octocrab = octocrab::instance();
        let mut page_index = 0u32;
        let mut releases = HashMap::default();

        // we can make this faster by fetching the pages in parallel
        loop {
            let page = octocrab
                .repos(owner, repo)
                .releases()
                .list()
                .per_page(100)
                .page(page_index)
                .send()
                .await
                .map_err(ListError::Github)?;

            if page.items.is_empty() {
                break;
            }

            for release in page {
                let solc = release
                    .assets
                    .into_iter()
                    .find(|asset| asset.name == "solc");
                if let Some(solc) = solc {
                    let version =
                        CompilerVersion::from_str(&release.tag_name).map_err(ListError::TagName)?;
                    releases.insert(version, solc.browser_download_url);
                }
            }

            page_index += 1;
        }

        Ok(releases)
    }

    pub async fn new(owner: &str, repo: &str, folder: PathBuf) -> Result<Self, ListError> {
        Ok(Self {
            releases: Self::list_releases(owner, repo).await?,
            folder,
        })
    }
}

#[derive(Error, Debug)]
pub enum FetchError {
    #[error("version {0} not found")]
    NotFound(CompilerVersion),
    #[error("couldn't fetch the file: {0}")]
    Fetch(reqwest::Error),
    #[error("couldn't create file: {0}")]
    File(std::io::Error),
    #[error("tokio sheduling error: {0}")]
    Shedule(tokio::task::JoinError),
}

#[cfg(target_family = "unix")]
fn create_executable(path: &Path) -> Result<File, std::io::Error> {
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o777)
        .open(path)
}

#[async_trait]
impl Fetcher for GithubFetcher {
    type Error = FetchError;
    async fn fetch(&self, ver: &CompilerVersion) -> Result<PathBuf, Self::Error> {
        let url = self
            .releases
            .get(ver)
            .ok_or_else(|| FetchError::NotFound(ver.clone()))?;

        let response = reqwest::get(url.clone()).await.map_err(FetchError::Fetch)?;
        let folder = self.folder.join(ver.to_string());
        let file = folder.join("solc");
        let bytes = response.bytes().await.map_err(FetchError::Fetch)?;
        {
            let file = file.clone();
            tokio::task::spawn_blocking(move || -> Result<(), Self::Error> {
                std::fs::create_dir_all(&folder).map_err(FetchError::File)?;
                std::fs::remove_file(file.as_path())
                    .or_else(|e| {
                        if e.kind() == ErrorKind::NotFound {
                            Ok(())
                        } else {
                            Err(e)
                        }
                    })
                    .map_err(FetchError::File)?;
                let mut file = create_executable(file.as_path()).map_err(FetchError::File)?;
                std::io::copy(&mut bytes.as_ref(), &mut file).map_err(FetchError::File)?;
                Ok(())
            })
            .await
            .map_err(FetchError::Shedule)??;
        }

        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers_solc::Solc;

    fn assert_has_version(releases: &HashMap<CompilerVersion, Url>, ver: &str, expect: &str) {
        let ver = CompilerVersion::from_str(ver).unwrap();
        let solc = releases.get(&ver).unwrap();
        let url = solc.to_string();
        assert_eq!(url, expect, "urls don't match");
    }

    #[tokio::test]
    async fn list_releases() {
        let releases = GithubFetcher::list_releases("blockscout", "solc-bin")
            .await
            .unwrap();

        // random release
        assert_has_version(&releases,
            "solc-v0.8.3+commit.8d00100c",
            "https://github.com/blockscout/solc-bin/releases/download/solc-v0.8.3%2Bcommit.8d00100c/solc");
        // random nightly
        assert_has_version(&releases,
            "solc-v0.8.4-nightly.2021.4.14+commit.69411436",
            "https://github.com/blockscout/solc-bin/releases/download/solc-v0.8.4-nightly.2021.4.14%2Bcommit.69411436/solc");
        // first in the list
        assert_has_version(&releases,
            "solc-v0.8.9+commit.e5eed63a",
            "https://github.com/blockscout/solc-bin/releases/download/solc-v0.8.9%2Bcommit.e5eed63a/solc");
        // last in the list
        assert_has_version(&releases,
            "solc-v0.5.0+commit.1d4f565a",
            "https://github.com/blockscout/solc-bin/releases/download/solc-v0.5.0%2Bcommit.1d4f565a/solc");
    }

    #[tokio::test]
    async fn download_release() {
        let version = CompilerVersion::from_str("solc-v0.8.3+commit.8d00100c").unwrap();
        let url = Url::from_str("https://github.com/blockscout/solc-bin/releases/download/solc-v0.8.3%2Bcommit.8d00100c/solc").unwrap();
        let fetcher = GithubFetcher {
            releases: HashMap::from([(version.clone(), url)]),
            folder: std::env::temp_dir().join("blockscout/verification/github_fetcher/test/"),
        };
        let file = fetcher.fetch(&version).await.unwrap();
        let solc = Solc::new(file);
        let ver = solc.version().unwrap();
        assert_eq!((ver.major, ver.minor, ver.patch), (0, 8, 3));
    }
}
