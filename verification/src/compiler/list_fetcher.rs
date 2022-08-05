use super::fetcher::FetchError;
use crate::{
    compiler::{Fetcher, Version},
    scheduler,
    types::Mismatch,
};
use async_trait::async_trait;
use bytes::Bytes;
use cron::Schedule;
use primitive_types::H256;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fmt::Debug,
    fs::{File, OpenOptions},
    io::ErrorKind,
    os::unix::prelude::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;
use url::Url;

mod json {
    use crate::compiler;
    use primitive_types::H256;
    use serde::{Deserialize, Serialize};
    use url::Url;

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    pub struct List {
        pub builds: Vec<CompilerInfo>,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct CompilerInfo {
        pub path: DownloadPath,
        #[serde(with = "serde_with::rust::display_fromstr")]
        pub long_version: compiler::Version,
        pub sha256: H256,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    #[serde(untagged)]
    pub enum DownloadPath {
        Url(Url),
        Filename(String),
    }
}

type VersionsMap = HashMap<Version, CompilerInfo>;

#[derive(Debug, PartialEq, Clone)]
struct CompilerInfo {
    pub url: Url,
    pub sha256: H256,
}

#[derive(Error, Debug)]
pub enum ListError {
    #[error("fetching list json returned error: {0}")]
    ListJsonFetch(reqwest::Error),
    #[error("cannot parse list json file: {0}")]
    ParseListJson(reqwest::Error),
    #[error("error parsing 'path' field: {0}")]
    Path(url::ParseError),
}

async fn try_fetch_versions(versions_list_url: &Url) -> Result<VersionsMap, ListError> {
    let list_json_file: json::List = reqwest::get(versions_list_url.as_str())
        .await
        .map_err(ListError::ListJsonFetch)?
        .json()
        .await
        .map_err(ListError::ParseListJson)?;
    try_parse_json_file(list_json_file, versions_list_url)
}

fn try_parse_json_file(
    list_json_file: json::List,
    versions_list_url: &Url,
) -> Result<VersionsMap, ListError> {
    let mut compiler_versions = HashMap::default();
    for json_compiler_info in list_json_file.builds {
        let version = json_compiler_info.long_version.clone();
        let compiler_info = CompilerInfo::try_from((json_compiler_info, versions_list_url))
            .map_err(ListError::Path)?;
        compiler_versions.insert(version, compiler_info);
    }
    Ok(compiler_versions)
}

impl TryFrom<(json::CompilerInfo, &Url)> for CompilerInfo {
    type Error = url::ParseError;

    fn try_from(
        (compiler_info, download_url): (json::CompilerInfo, &Url),
    ) -> Result<Self, Self::Error> {
        let url = match compiler_info.path {
            json::DownloadPath::Url(url) => url,
            // download_url ends with `.../list.json` but join() will replace this with `filename`
            json::DownloadPath::Filename(filename) => download_url.join(&filename)?,
        };
        Ok(Self {
            url,
            sha256: compiler_info.sha256,
        })
    }
}

#[derive(Default, Clone)]
struct Versions(Arc<parking_lot::RwLock<VersionsMap>>);

impl Versions {
    fn spawn_refresh_job(self, versions_list_url: Url, cron_schedule: Schedule) {
        log::info!("spawn version refresh job");
        scheduler::spawn_job(cron_schedule, "refresh compiler versions", move || {
            let versions_list_url = versions_list_url.clone();
            let versions = self.clone();
            async move {
                let refresh_result = versions.refresh_versions(&versions_list_url).await;
                if let Err(err) = refresh_result {
                    log::error!("error during version refresh: {}", err);
                };
            }
        });
    }

    async fn refresh_versions(&self, versions_list_url: &Url) -> anyhow::Result<()> {
        log::info!("looking for new compilers versions");
        let fetched_versions = try_fetch_versions(versions_list_url)
            .await
            .map_err(anyhow::Error::msg)?;
        let need_to_update = {
            let versions = self.0.read();
            fetched_versions != *versions
        };
        if need_to_update {
            let (old_len, new_len) = {
                // we don't need to check condition again,
                // we can just override the value
                let mut versions = self.0.write();
                let old_len = versions.len();
                *versions = fetched_versions;
                let new_len = versions.len();
                (old_len, new_len)
            };
            log::info!(
                "found new compiler versions. old length: {}, new length: {}",
                old_len,
                new_len,
            );
        } else {
            log::info!("no new versions found")
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct ListFetcher {
    compiler_versions: Versions,
    folder: PathBuf,
}

impl ListFetcher {
    pub async fn new(
        versions_list_url: Url,
        refresh_versions_schedule: Option<Schedule>,
        folder: PathBuf,
    ) -> anyhow::Result<Self> {
        let compiler_versions = try_fetch_versions(&versions_list_url)
            .await
            .map_err(anyhow::Error::msg)?;
        let compiler_versions = Versions(Arc::new(parking_lot::RwLock::new(compiler_versions)));
        if let Some(cron_schedule) = refresh_versions_schedule {
            compiler_versions
                .clone()
                .spawn_refresh_job(versions_list_url.clone(), cron_schedule)
        }
        Ok(Self {
            compiler_versions,
            folder,
        })
    }
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

pub fn check_hashsum(bytes: &Bytes, expected: H256) -> Result<(), Mismatch<H256>> {
    let start = std::time::Instant::now();

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let found = H256::from_slice(&hasher.finalize());

    let took = std::time::Instant::now() - start;
    log::debug!("check hashsum of {} bytes took {:?}", bytes.len(), took,);
    if expected != found {
        Err(Mismatch::new(expected, found))
    } else {
        Ok(())
    }
}

#[async_trait]
impl Fetcher for ListFetcher {
    async fn fetch(&self, ver: &Version) -> Result<PathBuf, FetchError> {
        let compiler_info = {
            let compiler_versions = self.compiler_versions.0.read();
            let compiler_info = compiler_versions
                .get(ver)
                .ok_or_else(|| FetchError::NotFound(ver.clone()))?;
            (*compiler_info).clone()
        };

        let response = reqwest::get(compiler_info.url.to_string())
            .await
            .map_err(anyhow::Error::msg)?;
        let folder = self.folder.join(ver.to_string());
        let file = folder.join("solc");
        let bytes = response.bytes().await.map_err(anyhow::Error::msg)?;

        let save_result = {
            let file = file.clone();
            let bytes = bytes.clone();
            tokio::task::spawn_blocking(move || -> Result<(), FetchError> {
                std::fs::create_dir_all(&folder)?;
                std::fs::remove_file(file.as_path()).or_else(|e| {
                    if e.kind() == ErrorKind::NotFound {
                        Ok(())
                    } else {
                        Err(e)
                    }
                })?;
                let mut file = create_executable(file.as_path())?;
                std::io::copy(&mut bytes.as_ref(), &mut file)?;
                Ok(())
            })
        };

        let check_result =
            tokio::task::spawn_blocking(move || check_hashsum(&bytes, compiler_info.sha256));

        check_result.await??;
        save_result.await??;

        Ok(file)
    }

    fn all_versions(&self) -> Vec<Version> {
        let compiler_versions = self.compiler_versions.0.read();
        compiler_versions
            .iter()
            .map(|(ver, _)| ver.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{tests::parse::test_deserialize_ok, Config};
    use ethers_solc::Solc;
    use pretty_assertions::assert_eq;
    use std::{env::temp_dir, str::FromStr};
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    const DEFAULT_LIST_JSON: &str = r#"{
        "builds": [
            {
                "path": "https://github.com/blockscout/solc-bin/releases/download/solc-v0.8.15-nightly.2022.5.27%2Bcommit.095cc647/solc",
                "longVersion": "0.8.15-nightly.2022.5.27+commit.095cc647",
                "sha256": "35708c1593f3daddae734065e361a839ee39d400825972fb3f50718495be82b1"
            },
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
        let ver = |s| Version::from_str(s).unwrap();
        test_deserialize_ok(vec![
            (DEFAULT_LIST_JSON,
            json::List {
                builds: vec![
                    json::CompilerInfo {
                        path: json::DownloadPath::Url(Url::from_str("https://github.com/blockscout/solc-bin/releases/download/solc-v0.8.15-nightly.2022.5.27%2Bcommit.095cc647/solc").unwrap()),
                        long_version: ver("0.8.15-nightly.2022.5.27+commit.095cc647"),
                        sha256: H256::from_str("35708c1593f3daddae734065e361a839ee39d400825972fb3f50718495be82b1").unwrap(),
                    },
                    json::CompilerInfo {
                        path: json::DownloadPath::Url(Url::from_str("https://binaries.soliditylang.org/linux-amd64/solc-linux-amd64-v0.4.13+commit.0fb4cb1a").unwrap()),
                        long_version: ver("0.4.13+commit.0fb4cb1a"),
                        sha256: H256::from_str("0x791ee3a20adf6c5ab76cc889f13cca102f76eb0b7cf0da4a0b5b11dc46edf349").unwrap(),
                    },
                    json::CompilerInfo {
                        path: json::DownloadPath::Url(Url::from_str("https://binaries.soliditylang.org/linux-amd64/solc-linux-amd64-v0.4.14+commit.c2215d46").unwrap()),
                        long_version: ver("0.4.14+commit.c2215d46"),
                        sha256: H256::from_str("0x28ce35a0941d9ecd59a2b1a377c019110e79a6b38bdbf5a3bffea811f9c2a13b").unwrap(),
                    },
                    json::CompilerInfo {
                        path: json::DownloadPath::Filename("solc-linux-amd64-v0.4.15+commit.8b45bddb".to_string()),
                        long_version: ver("0.4.15+commit.8b45bddb"),
                        sha256: H256::from_str("0xc71ac6c28bf3b1a425e77e97f5df67a80da3e4c047261875206561c0a110c0cb").unwrap(),
                    },
                    json::CompilerInfo {
                        path: json::DownloadPath::Filename("download/files/solc-linux-amd64-v0.4.16+commit.d7661dd9".to_string()),
                        long_version: ver("0.4.16+commit.d7661dd9"),
                        sha256: H256::from_str("0x78e0da6cad24ab145a8d17420c4f094c8314418ca23cff4b050bb2bfd36f3af2").unwrap(),
                    },
                    json::CompilerInfo {
                        path: json::DownloadPath::Filename("solc-linux-amd64-v10.8.9-nightly.2021.9.11+commit.e5eed63a".to_string()),
                        long_version: ver("10.8.9-nightly.2021.9.11+commit.e5eed63a"),
                        sha256: H256::from_str("0x791ee3a20adf6c5ab76cc889f13cca102f76eb0b7cf0da4a0b5b11dc46edf349").unwrap(),
                    },
                ]
            })
        ]);
    }

    fn assert_has_version(versions: &VersionsMap, ver: &str, expect: &str) {
        let ver = Version::from_str(ver).unwrap();
        let info = versions.get(&ver).unwrap();
        let url = info.url.to_string();
        assert_eq!(url, expect, "urls don't match");
    }

    #[test]
    fn parse_versions() {
        let list_json_file: json::List = serde_json::from_str(DEFAULT_LIST_JSON).unwrap();
        let download_url = Url::from_str(DEFAULT_DOWNLOAD_PREFIX).expect("valid url");
        let verions = try_parse_json_file(list_json_file, &download_url).unwrap();
        assert_has_version(
            &verions,
            "0.8.15-nightly.2022.5.27+commit.095cc647",
            "https://github.com/blockscout/solc-bin/releases/download/solc-v0.8.15-nightly.2022.5.27%2Bcommit.095cc647/solc",
        );
        assert_has_version(
            &verions,
            "0.4.13+commit.0fb4cb1a",
            "https://binaries.soliditylang.org/linux-amd64/solc-linux-amd64-v0.4.13+commit.0fb4cb1a",
        );
        assert_has_version(&verions,
            "10.8.9-nightly.2021.9.11+commit.e5eed63a",
            "https://binaries.soliditylang.org/linux-amd64/solc-linux-amd64-v10.8.9-nightly.2021.9.11+commit.e5eed63a"
        );
        assert_has_version(&verions,
            "0.4.16+commit.d7661dd9",
            "https://binaries.soliditylang.org/linux-amd64/download/files/solc-linux-amd64-v0.4.16+commit.d7661dd9"
        );
    }

    #[tokio::test]
    async fn list_download_versions() {
        let config = Config::default();
        let fetcher = ListFetcher::new(
            config.solidity.compilers_list_url,
            None,
            std::env::temp_dir().join("blockscout/verification/compiler_fetcher/test/"),
        )
        .await
        .expect("list.json file should be valid");

        for compiler_version in vec![
            Version::from_str("0.7.0+commit.9e61f92b").unwrap(),
            Version::from_str("0.8.9+commit.e5eed63a").unwrap(),
        ] {
            let file = fetcher.fetch(&compiler_version).await.unwrap();
            let solc = Solc::new(file);
            let ver = solc.version().unwrap();
            assert_eq!(
                (ver.major, ver.minor, ver.patch),
                (
                    compiler_version.version().major,
                    compiler_version.version().minor,
                    compiler_version.version().patch,
                )
            );
        }
    }

    #[tokio::test]
    async fn check_refresh_versions() {
        let mock_server = MockServer::start().await;

        // mock list.json server response with empty list
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes("{\"builds\": []}"))
            .mount(&mock_server)
            .await;
        let fetcher = ListFetcher::new(
            Url::parse(&mock_server.uri()).unwrap(),
            Some(Schedule::from_str("* * * * * * *").unwrap()),
            temp_dir(),
        )
        .await
        .expect("cannot initialize fetcher");
        assert!(fetcher.all_versions().is_empty());

        // mock list.json server response with `DEFAULT_LIST_JSON`
        mock_server.reset().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(DEFAULT_LIST_JSON))
            .mount(&mock_server)
            .await;
        // wait for refresher to do its job
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
        let versions = fetcher.all_versions();
        assert!(
            versions.contains(&Version::from_str("0.4.13+commit.0fb4cb1a").unwrap()),
            "versions list doesn't have 0.4.13: {versions:?}",
        );
    }

    const VYPER_LIST_JSON: &str = r#"{
        "builds": [
            {
                "path": "https://github.com/vyperlang/vyper/releases/download/v0.3.2/vyper.0.3.2%2Bcommit.3b6a4117.linux",
                "longVersion": "0.3.2+commit.3b6a4117",
                "sha256": "7101527cc0976468a07087e98438e88e372c02002a5b8c8c6c411517176c2592"
            }
        ]
    }"#;

    /// That's will try to download the Vyper compiler from the list.json file.
    /// It check's:
    /// 1) an access to a download link
    /// 2) Hash (mis)matching
    #[tokio::test]
    async fn download_vyper_versions() {
        let mock_server = MockServer::start().await;

        // mock list.json server response with empty list
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(VYPER_LIST_JSON))
            .mount(&mock_server)
            .await;
        let fetcher = ListFetcher::new(Url::parse(&mock_server.uri()).unwrap(), None, temp_dir())
            .await
            .expect("cannot initialize fetcher");

        let versions = fetcher.all_versions();
        assert!(
            versions.contains(&Version::from_str("0.3.2+commit.3b6a4117").unwrap()),
            "versions list doesn't have 0.3.2: {versions:?}",
        );

        for compiler_version in versions {
            fetcher.fetch(&compiler_version).await.expect(
                format!(
                    "fetcher: can't download vyper compiler {}",
                    compiler_version
                )
                .as_str(),
            );
        }
    }
}
