use crate::compiler::{
    fetcher::Fetcher,
    version::{CompilerVersion, ParseError},
};
use async_trait::async_trait;
use serde::Deserialize;
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

#[derive(Error, Debug)]
pub enum ListError {
    #[error("fetching list json returned error: {0}")]
    ListJsonFetch(reqwest::Error),
    #[error("cannot parse list json file: {0}")]
    ParseListJson(reqwest::Error),
    #[error("error parsing tag name into version: {0}")]
    TagName(ParseError),
    #[error("error parsing 'path' field: {0}")]
    Path(url::ParseError),
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct ListJson {
    pub builds: Vec<CompilerInfo>,
}

impl ListJson {
    fn into_releases(
        self,
        download_prefix: &Url,
    ) -> Result<HashMap<CompilerVersion, Url>, ListError> {
        let mut releases = HashMap::default();
        for build in self.builds {
            let version =
                CompilerVersion::from_str(&build.long_version).map_err(ListError::TagName)?;
            let download_url = build
                .path
                .try_into_url(download_prefix)
                .map_err(ListError::Path)?;
            releases.insert(version, download_url);
        }
        Ok(releases)
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompilerInfo {
    pub path: DownloadPath,
    pub long_version: String,
    pub sha256: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum DownloadPath {
    Url(Url),
    Filename(String),
}

impl DownloadPath {
    fn try_into_url(self, prefix: &Url) -> Result<Url, url::ParseError> {
        match self {
            DownloadPath::Url(url) => Ok(url),
            DownloadPath::Filename(filename) => prefix.join(&filename),
        }
    }
}

#[derive(Default)]
pub struct CompilerFetcher {
    releases: HashMap<CompilerVersion, Url>,
    folder: PathBuf,
}

impl CompilerFetcher {
    async fn list_releases(
        download_prefix: &Url,
    ) -> Result<HashMap<CompilerVersion, Url>, ListError> {
        let list_json_url = download_prefix.join("list.json").expect("valid url");
        let list_json_file: ListJson = reqwest::get(list_json_url)
            .await
            .map_err(ListError::ListJsonFetch)?
            .json()
            .await
            .map_err(ListError::ParseListJson)?;

        list_json_file.into_releases(download_prefix)
    }

    pub async fn new(compilers_list_url: &Url, folder: PathBuf) -> Result<Self, ListError> {
        Ok(Self {
            releases: Self::list_releases(compilers_list_url).await?,
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
impl Fetcher for CompilerFetcher {
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
    use crate::{tests::parse::test_deserialize_ok, Config};

    use super::*;
    use ethers_solc::Solc;

    const DEFAULT_LIST_JSON: &str = r#"{
        "builds": [
            {
                "path": "https://binaries.soliditylang.org/linux-amd64/solc-linux-amd64-v0.4.13+commit.0fb4cb1a",
                "longVersion": "0.4.13+commit.0fb4cb1a",
                "sha256": "0x791ee3a20adf6c5ab76cc889f13cca102f76eb0b7cf0da4a0b5b11dc46edf349"
            },
            {
                "path": "https://binaries.soliditylang.org/linux-amd64/solc-linux-amd64-v0.4.14+commit.c2215d46",
                "longVersion": "0.4.14+commit.c2215d46",
                "sha256": "0x28ce35a0941d9ecd59a2b1a377c019110e79a6b38bdbf5a3bffea811f9c2a13b"
            },
            {
                "path": "solc-linux-amd64-v0.4.15+commit.8b45bddb",
                "longVersion": "0.4.15+commit.8b45bddb",
                "sha256": "0xc71ac6c28bf3b1a425e77e97f5df67a80da3e4c047261875206561c0a110c0cb"
            },
            {
                "path": "download/files/solc-linux-amd64-v0.4.16+commit.d7661dd9",
                "longVersion": "0.4.16+commit.d7661dd9",
                "sha256": "0x78e0da6cad24ab145a8d17420c4f094c8314418ca23cff4b050bb2bfd36f3af2"
            },
            {
                "path": "solc-linux-amd64-v10.8.9-nightly.2021.9.11+commit.e5eed63a",
                "longVersion": "10.8.9-nightly.2021.9.11+commit.e5eed63a",
                "sha256": "0x791ee3a20adf6c5ab76cc889f13cca102f76eb0b7cf0da4a0b5b11dc46edf349"
            }
        ]
    }"#;
    const DEFAULT_DOWNLOAD_PREFIX: &str = "https://binaries.soliditylang.org/linux-amd64/";

    #[test]
    fn parse_list_json() {
        test_deserialize_ok(vec![
            (DEFAULT_LIST_JSON,
            ListJson{
                builds: vec![
                    CompilerInfo {
                        path: DownloadPath::Url(Url::from_str("https://binaries.soliditylang.org/linux-amd64/solc-linux-amd64-v0.4.13+commit.0fb4cb1a").expect("valid url")),
                        long_version: "0.4.13+commit.0fb4cb1a".to_string(),
                        sha256: "0x791ee3a20adf6c5ab76cc889f13cca102f76eb0b7cf0da4a0b5b11dc46edf349".to_string()
                    },
                    CompilerInfo {
                        path: DownloadPath::Url(Url::from_str("https://binaries.soliditylang.org/linux-amd64/solc-linux-amd64-v0.4.14+commit.c2215d46").expect("valid url")),
                        long_version: "0.4.14+commit.c2215d46".to_string(),
                        sha256: "0x28ce35a0941d9ecd59a2b1a377c019110e79a6b38bdbf5a3bffea811f9c2a13b".to_string()
                    },
                    CompilerInfo {
                        path: DownloadPath::Filename("solc-linux-amd64-v0.4.15+commit.8b45bddb".to_string()),
                        long_version: "0.4.15+commit.8b45bddb".to_string(),
                        sha256: "0xc71ac6c28bf3b1a425e77e97f5df67a80da3e4c047261875206561c0a110c0cb".to_string()
                    },
                    CompilerInfo {
                        path: DownloadPath::Filename("download/files/solc-linux-amd64-v0.4.16+commit.d7661dd9".to_string()),
                        long_version: "0.4.16+commit.d7661dd9".to_string(),
                        sha256: "0x78e0da6cad24ab145a8d17420c4f094c8314418ca23cff4b050bb2bfd36f3af2".to_string()
                    },
                    CompilerInfo {
                        path: DownloadPath::Filename("solc-linux-amd64-v10.8.9-nightly.2021.9.11+commit.e5eed63a".to_string()),
                        long_version: "10.8.9-nightly.2021.9.11+commit.e5eed63a".to_string(),
                        sha256: "0x791ee3a20adf6c5ab76cc889f13cca102f76eb0b7cf0da4a0b5b11dc46edf349".to_string()
                    },
                ]
            })
        ]);
    }

    fn assert_has_version(releases: &HashMap<CompilerVersion, Url>, ver: &str, expect: &str) {
        let ver = CompilerVersion::from_str(ver).unwrap();
        let solc = releases.get(&ver).unwrap();
        let url = solc.to_string();
        assert_eq!(url, expect, "urls don't match");
    }

    #[test]
    fn parse_releases() {
        let list_json_file: ListJson = serde_json::from_str(DEFAULT_LIST_JSON).unwrap();
        let download_url = Url::from_str(DEFAULT_DOWNLOAD_PREFIX).expect("valid url");
        let releases = list_json_file.into_releases(&download_url).unwrap();
        assert_has_version(
            &releases,
            "0.4.13+commit.0fb4cb1a",
            "https://binaries.soliditylang.org/linux-amd64/solc-linux-amd64-v0.4.13+commit.0fb4cb1a",
        );
        assert_has_version(&releases,
            "10.8.9-nightly.2021.9.11+commit.e5eed63a",
            "https://binaries.soliditylang.org/linux-amd64/solc-linux-amd64-v10.8.9-nightly.2021.9.11+commit.e5eed63a"
        );
        assert_has_version(&releases,
            "0.4.16+commit.d7661dd9",
            "https://binaries.soliditylang.org/linux-amd64/download/files/solc-linux-amd64-v0.4.16+commit.d7661dd9"
        );
    }

    async fn test_fetched_ok(fetcher: &CompilerFetcher, compiler_version: &CompilerVersion) {
        let file = fetcher.fetch(&compiler_version).await.unwrap();
        let solc = Solc::new(file);
        let ver = solc.version().unwrap();
        let (x, y, z) = (
            compiler_version.version().major,
            compiler_version.version().minor,
            compiler_version.version().patch,
        );
        assert_eq!((ver.major, ver.minor, ver.patch), (x, y, z));
    }

    #[tokio::test]
    async fn download_release() {
        let version = CompilerVersion::from_str("0.5.0+commit.1d4f565a").unwrap();
        let url = Url::from_str("https://github.com/blockscout/solc-bin/releases/download/solc-v0.5.0%2Bcommit.1d4f565a/solc").unwrap();
        let fetcher = CompilerFetcher {
            releases: HashMap::from([(version.clone(), url)]),
            folder: std::env::temp_dir().join("blockscout/verification/compiler_fetcher/test/"),
        };
        test_fetched_ok(&fetcher, &version).await;
    }

    #[tokio::test]
    async fn list_releases() {
        let config = Config::default();
        let fetcher = CompilerFetcher::new(
            &config.compiler.compilers_list_url,
            std::env::temp_dir().join("blockscout/verification/compiler_fetcher/test/"),
        )
        .await
        .expect("default list.json file should be valid");

        let version = CompilerVersion::from_str("0.7.0+commit.9e61f92b").unwrap();
        test_fetched_ok(&fetcher, &version).await;
    }
}
