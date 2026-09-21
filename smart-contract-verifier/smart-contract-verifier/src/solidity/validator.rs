// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::compiler::{
    CommandArgument, CompilerExecutor, CompilerInvocation, FileValidator, JobFile, Version,
};
use anyhow::{Context, Error};
use async_trait::async_trait;
use std::{path::Path, str::FromStr, sync::Arc};

#[derive(Clone)]
pub struct SolcValidator {
    executor: Arc<dyn CompilerExecutor>,
}

impl SolcValidator {
    pub fn new(executor: Arc<dyn CompilerExecutor>) -> Self {
        Self { executor }
    }
}

#[async_trait]
impl<Ver: Version> FileValidator<Ver> for SolcValidator {
    async fn validate(&self, ver: &Ver, path: &Path) -> Result<(), Error> {
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/solc", path)
                .context("building solc version invocation")?,
            vec![CommandArgument::literal("--version")],
            Vec::new(),
        );
        let output = self
            .executor
            .execute(invocation)
            .await
            .context("executing solc version probe")?;
        output
            .ensure_success("solc --version")
            .context("could not get compiler version")?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = stdout
            .lines()
            .rfind(|line| !line.trim().is_empty())
            .context("version not found in solc output")?
            .trim_start_matches("Version: ")
            .replace(".g++", ".gcc");
        let solc_ver =
            semver::Version::from_str(&version).context("parsing version from solc output")?;
        // ignore build and pre metadata
        let solc_ver = semver::Version::new(solc_ver.major, solc_ver.minor, solc_ver.patch);

        if &solc_ver != ver.to_semver() {
            Err(anyhow::anyhow!(
                "versions don't match: expected={}, got={}",
                ver.to_semver(),
                solc_ver
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        compiler::{DetailedVersion, Fetcher, ListFetcher, NativeCompilerExecutor},
        consts::DEFAULT_SOLIDITY_COMPILER_LIST,
    };
    use std::{
        fs::OpenOptions, io::Write, os::unix::prelude::OpenOptionsExt, path::PathBuf, str::FromStr,
    };
    use tokio::sync::OnceCell;

    fn default_version() -> DetailedVersion {
        DetailedVersion::from_str("v0.8.9+commit.e5eed63a").unwrap()
    }

    fn native_validator() -> SolcValidator {
        SolcValidator::new(Arc::new(NativeCompilerExecutor::default()))
    }

    async fn fetch_compiler() -> PathBuf {
        static COMPILERS: OnceCell<PathBuf> = OnceCell::const_new();
        COMPILERS
            .get_or_init(|| async {
                let tmp_dir = tempfile::tempdir().unwrap();
                let url = DEFAULT_SOLIDITY_COMPILER_LIST.try_into().unwrap();
                let fetcher = ListFetcher::new(url, tmp_dir.into_path(), None, None)
                    .await
                    .expect("Fetch releases");
                fetcher.fetch(&default_version()).await.unwrap()
            })
            .await
            .clone()
    }

    #[tokio::test]
    async fn success() {
        let compiler = fetch_compiler().await;
        let validator = native_validator();
        validator
            .validate(&default_version(), compiler.as_path())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn wrong_version() {
        let compiler = fetch_compiler().await;
        let validator = native_validator();
        let other_ver = DetailedVersion::from_str("v0.8.10+commit.e5eed63a").unwrap();
        validator
            .validate(&other_ver, compiler.as_path())
            .await
            .expect_err("expected version mismatch");
    }

    #[cfg(target_family = "unix")]
    #[tokio::test]
    async fn corrupted_binary() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let compiler = tmp_dir.path().join("wrong_solc");
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o777)
            .open(compiler.clone())
            .unwrap();
        file.write_all(b"This isn't a compiler").unwrap();

        let validator = native_validator();
        validator
            .validate(&default_version(), compiler.as_path())
            .await
            .expect_err("expected failing to execute file");
    }
}
