use crate::compiler::{fetcher::Fetcher, version::CompilerVersion};
use async_trait::async_trait;
use primitive_types::H256;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::ErrorKind,
    os::unix::prelude::OpenOptionsExt,
    path::{Path, PathBuf},
};
use thiserror::Error;
use url::Url;

use self::types::{CompilerInfo, ListJson};

#[derive(Error, Debug)]
pub enum ListError {
    #[error("fetching list json returned error: {0}")]
    ListJsonFetch(reqwest::Error),
    #[error("cannot parse list json file: {0}")]
    ParseListJson(reqwest::Error),
    #[error("error parsing 'path' field: {0}")]
    Path(url::ParseError),
}

#[derive(Default)]
pub struct CompilerFetcher {
    releases: HashMap<CompilerVersion, CompilerInfo>,
    folder: PathBuf,
}

impl CompilerFetcher {
    pub async fn new(compilers_list_url: &Url, folder: PathBuf) -> Result<Self, ListError> {
        Ok(Self {
            releases: Self::list_releases(compilers_list_url).await?,
            folder,
        })
    }

    async fn list_releases(
        compilers_list_url: &Url,
    ) -> Result<HashMap<CompilerVersion, CompilerInfo>, ListError> {
        let list_json_file: ListJson = reqwest::get(compilers_list_url.to_string())
            .await
            .map_err(ListError::ListJsonFetch)?
            .json()
            .await
            .map_err(ListError::ParseListJson)?;

        list_json_file
            .into_releases(compilers_list_url)
            .map_err(ListError::Path)
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
        let compiler_info = self
            .releases
            .get(ver)
            .ok_or_else(|| FetchError::NotFound(ver.clone()))?;

        let response = reqwest::get(compiler_info.url.clone())
            .await
            .map_err(FetchError::Fetch)?;
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

mod types {
    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    pub struct ListJson {
        pub builds: Vec<DeserializedCompilerInfo>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct DeserializedCompilerInfo {
        pub path: DownloadPath,
        #[serde(with = "serde_with::rust::display_fromstr")]
        pub long_version: CompilerVersion,
        pub sha256: H256,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(untagged)]
    pub enum DownloadPath {
        Url(Url),
        Filename(String),
    }

    #[derive(Debug)]
    pub struct CompilerInfo {
        pub url: Url,
        pub sha256: H256,
    }

    impl ListJson {
        pub fn into_releases(
            self,
            download_url: &Url,
        ) -> Result<HashMap<CompilerVersion, CompilerInfo>, url::ParseError> {
            let mut releases = HashMap::default();
            for deserialized_info in self.builds {
                let version = deserialized_info.long_version.clone();
                let compiler_info = CompilerInfo::try_from((deserialized_info, download_url))?;
                releases.insert(version, compiler_info);
            }
            Ok(releases)
        }
    }

    impl TryFrom<(DeserializedCompilerInfo, &Url)> for CompilerInfo {
        type Error = url::ParseError;

        fn try_from(
            (compiler_info, download_url): (DeserializedCompilerInfo, &Url),
        ) -> Result<Self, Self::Error> {
            let url = match compiler_info.path {
                DownloadPath::Url(url) => url,
                // download_url ends with `.../list.json` but join() will replace this with `filename`
                DownloadPath::Filename(filename) => download_url.join(&filename)?,
            };
            Ok(Self {
                url,
                sha256: compiler_info.sha256,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{tests::parse::test_deserialize_ok, Config};

    use super::{types::*, *};
    use ethers_solc::Solc;
    use std::str::FromStr;

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
        let ver = |s| CompilerVersion::from_str(s).unwrap();
        test_deserialize_ok(vec![
            (DEFAULT_LIST_JSON,
            ListJson{
                builds: vec![
                    DeserializedCompilerInfo {
                        path: DownloadPath::Url(Url::from_str("https://github.com/blockscout/solc-bin/releases/download/solc-v0.8.15-nightly.2022.5.27%2Bcommit.095cc647/solc").unwrap()),
                        long_version: ver("0.8.15-nightly.2022.5.27+commit.095cc647"),
                        sha256: H256::from_str("35708c1593f3daddae734065e361a839ee39d400825972fb3f50718495be82b1").unwrap(),
                    },
                    DeserializedCompilerInfo {
                        path: DownloadPath::Url(Url::from_str("https://binaries.soliditylang.org/linux-amd64/solc-linux-amd64-v0.4.13+commit.0fb4cb1a").unwrap()),
                        long_version: ver("0.4.13+commit.0fb4cb1a"),
                        sha256: H256::from_str("0x791ee3a20adf6c5ab76cc889f13cca102f76eb0b7cf0da4a0b5b11dc46edf349").unwrap(),
                    },
                    DeserializedCompilerInfo {
                        path: DownloadPath::Url(Url::from_str("https://binaries.soliditylang.org/linux-amd64/solc-linux-amd64-v0.4.14+commit.c2215d46").unwrap()),
                        long_version: ver("0.4.14+commit.c2215d46"),
                        sha256: H256::from_str("0x28ce35a0941d9ecd59a2b1a377c019110e79a6b38bdbf5a3bffea811f9c2a13b").unwrap(),
                    },
                    DeserializedCompilerInfo {
                        path: DownloadPath::Filename("solc-linux-amd64-v0.4.15+commit.8b45bddb".to_string()),
                        long_version: ver("0.4.15+commit.8b45bddb"),
                        sha256: H256::from_str("0xc71ac6c28bf3b1a425e77e97f5df67a80da3e4c047261875206561c0a110c0cb").unwrap(),
                    },
                    DeserializedCompilerInfo {
                        path: DownloadPath::Filename("download/files/solc-linux-amd64-v0.4.16+commit.d7661dd9".to_string()),
                        long_version: ver("0.4.16+commit.d7661dd9"),
                        sha256: H256::from_str("0x78e0da6cad24ab145a8d17420c4f094c8314418ca23cff4b050bb2bfd36f3af2").unwrap(),
                    },
                    DeserializedCompilerInfo {
                        path: DownloadPath::Filename("solc-linux-amd64-v10.8.9-nightly.2021.9.11+commit.e5eed63a".to_string()),
                        long_version: ver("10.8.9-nightly.2021.9.11+commit.e5eed63a"),
                        sha256: H256::from_str("0x791ee3a20adf6c5ab76cc889f13cca102f76eb0b7cf0da4a0b5b11dc46edf349").unwrap(),
                    },
                ]
            })
        ]);
    }

    fn assert_has_version(
        releases: &HashMap<CompilerVersion, CompilerInfo>,
        ver: &str,
        expect: &str,
    ) {
        let ver = CompilerVersion::from_str(ver).unwrap();
        let info = releases.get(&ver).unwrap();
        let url = info.url.to_string();
        assert_eq!(url, expect, "urls don't match");
    }

    #[test]
    fn parse_releases() {
        let list_json_file: ListJson = serde_json::from_str(DEFAULT_LIST_JSON).unwrap();
        let download_url = Url::from_str(DEFAULT_DOWNLOAD_PREFIX).expect("valid url");
        let releases = list_json_file.into_releases(&download_url).unwrap();
        assert_has_version(
            &releases,
            "0.8.15-nightly.2022.5.27+commit.095cc647",
            "https://github.com/blockscout/solc-bin/releases/download/solc-v0.8.15-nightly.2022.5.27%2Bcommit.095cc647/solc",
        );
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

    #[tokio::test]
    async fn list_download_releases() {
        env_logger::init();
        let config = Config::test().unwrap();
        let fetcher = CompilerFetcher::new(
            &config.solidity.compilers_list_url,
            std::env::temp_dir().join("blockscout/verification/compiler_fetcher/test/"),
        )
        .await
        .expect("list.json file should be valid");
        log::info!("{:?}", fetcher.releases);
        for compiler_version in vec![
            CompilerVersion::from_str("0.7.0+commit.9e61f92b").unwrap(),
            CompilerVersion::from_str("0.8.9+commit.e5eed63a").unwrap(),
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
}
