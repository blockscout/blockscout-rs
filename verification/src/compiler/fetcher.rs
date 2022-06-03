use super::version::CompilerVersion;
use async_trait::async_trait;
use std::path::PathBuf;

#[async_trait]
pub trait Fetcher {
    type Error;
    async fn fetch(&self, ver: &CompilerVersion) -> Result<PathBuf, Self::Error>;
}
