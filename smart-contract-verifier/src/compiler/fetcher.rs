use super::version::Version;
use crate::types::Mismatch;
use async_trait::async_trait;
use bytes::Bytes;
use primitive_types::H256;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::ErrorKind,
    os::unix::prelude::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;
use tracing::instrument;

#[derive(Error, Debug)]
pub enum FetchError {
    #[error("version {0} not found")]
    NotFound(Version),
    #[error("couldn't fetch the file: {0}")]
    Fetch(anyhow::Error),
    #[error("hashsum of fetched file mismatch: {0}")]
    HashMismatch(#[from] Mismatch<H256>),
    #[error("can't parse hashsum: {0}")]
    HashParse(anyhow::Error),
    #[error("couldn't create file: {0}")]
    File(#[from] std::io::Error),
    #[error("tokio sheduling error: {0}")]
    Schedule(#[from] tokio::task::JoinError),
    #[error("validation failed: {0}")]
    Validation(anyhow::Error),
}

#[async_trait]
pub trait FileValidator: Send + Sync {
    async fn validate(&self, ver: &Version, path: &Path) -> Result<(), anyhow::Error>;
}

#[async_trait]
pub trait Fetcher: Send + Sync {
    async fn fetch(&self, ver: &Version) -> Result<PathBuf, FetchError>;
    fn with_validator(&mut self, validator: Arc<dyn FileValidator>);
    fn all_versions(&self) -> Vec<Version>;
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

#[instrument(skip(bytes), level = "debug")]
pub fn validate_checksum(bytes: &Bytes, expected: H256) -> Result<(), Mismatch<H256>> {
    let found = Sha256::digest(bytes);
    let found = H256::from_slice(&found);
    if expected != found {
        Err(Mismatch::new(expected, found))
    } else {
        Ok(())
    }
}

pub async fn write_executable(
    data: Bytes,
    sha: H256,
    path: &Path,
    ver: &Version,
    validator: Option<&dyn FileValidator>,
) -> Result<PathBuf, FetchError> {
    let folder = path.join(ver.to_string());
    let file = folder.join("solc");

    let save_result = {
        let file = file.clone();
        let data = data.clone();
        let span = tracing::debug_span!("save executable");
        tokio::task::spawn_blocking(move || {
            let _guard = span.enter();
            std::fs::create_dir_all(&folder)?;
            std::fs::remove_file(file.as_path()).or_else(|e| {
                if e.kind() == ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(e)
                }
            })?;
            let mut file = create_executable(file.as_path())?;
            std::io::copy(&mut data.as_ref(), &mut file)
        })
    };
    let check_result = {
        let span = tracing::debug_span!("check hash result");
        tokio::task::spawn_blocking(move || {
            let _guard = span.enter();
            validate_checksum(&data, sha)
        })
    };

    let (check_result, save_result) = futures::join!(check_result, save_result);
    check_result??;
    save_result??;

    if let Some(validator) = validator {
        validator
            .validate(ver, file.as_path())
            .await
            .map_err(FetchError::Validation)?;
    }

    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test]
    async fn write_text_executable() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let data = "this is a compiler binary";
        let bytes = Bytes::from_static(data.as_bytes());
        let sha = Sha256::digest(data.as_bytes());
        let version = Version::from_str("v0.4.10+commit.f0d539ae").unwrap();
        let file = write_executable(
            bytes,
            H256::from_slice(&sha),
            tmp_dir.path(),
            &version,
            None,
        )
        .await
        .unwrap();
        let content = tokio::fs::read_to_string(file).await.unwrap();
        assert_eq!(data, content);
    }
}
