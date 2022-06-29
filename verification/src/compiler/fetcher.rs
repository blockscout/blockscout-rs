use super::version::CompilerVersion;
use async_trait::async_trait;
use std::path::PathBuf;

#[async_trait]
pub trait Fetcher {
    type Error: Send + Sync + 'static;
    async fn fetch(&self, ver: &CompilerVersion) -> Result<PathBuf, Self::Error>;
}

#[async_trait]
pub trait VersionList {
    async fn all_versions(&self) -> Vec<CompilerVersion>;
}
