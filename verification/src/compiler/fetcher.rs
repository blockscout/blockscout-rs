use super::version::CompilerVersion;
use async_trait::async_trait;
use std::path::PathBuf;

#[async_trait]
pub trait Fetcher {
    type Error: Send + Sync + 'static;
    async fn fetch(&self, ver: &CompilerVersion) -> Result<PathBuf, Self::Error>;
}

pub trait VersionList {
    fn all_versions(&self) -> Vec<CompilerVersion>;
}

/// Declare path to Solc Compilers home directory, "~/.solc_compilers" on Unix-based machines.
//
// May be implemented as once_cell::sync::Lazy to be initialized only once,
// but in that case home directory will be the same for all tests which may
// cause data race.
// For production usage the function would be called only once, thus no performance impact.
pub fn fetcher_home() -> PathBuf {
    cfg_if::cfg_if! {
        if #[cfg(test)] {
            let dir = tempfile::tempdir().expect("could not create temp directory");
            dir.path().join(".solc_compilers")
        } else {
            let mut user_home = home::home_dir().expect("could not detect user home directory");
            user_home.push(".solc_compilers");
            user_home
        }
    }
}
