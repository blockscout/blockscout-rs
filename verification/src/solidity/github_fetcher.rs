use crate::compiler::{fetcher::Fetcher, version::CompilerVersion};
use async_trait::async_trait;
use std::{collections::HashMap, path::PathBuf};
use thiserror::Error;

#[derive(Default)]
pub struct GithubFetcher {
    releases: HashMap<CompilerVersion, String>,
}

impl GithubFetcher {
    pub fn new() -> Result<Self, ()> {
        todo!("fetch all compilers");
    }
}

#[derive(Error, Debug)]
pub enum GithubFetchError {
    #[error("version {0} not found")]
    NotFound(CompilerVersion),
}

#[async_trait]
impl Fetcher for GithubFetcher {
    type Error = GithubFetchError;
    async fn fetch(&self, ver: &CompilerVersion) -> Result<PathBuf, Self::Error> {
        let url = self
            .releases
            .get(&ver)
            .ok_or(GithubFetchError::NotFound(ver.clone()))?;
        todo!("download file from url");
    }
}
