use super::{
    download_cache::DownloadCache,
    fetcher::{FetchError, Fetcher},
    version::Version,
};
use crate::metrics::{self, GuardedGauge};
use ethers_solc::{artifacts::Severity, error::SolcError, CompilerInput, CompilerOutput};
use std::{
    fmt::Debug,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;
use tokio::sync::{AcquireError, Semaphore};
use tracing::instrument;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Compiler version not found: {0}")]
    VersionNotFound(Version),
    #[error("Error while fetching compiler: {0:#}")]
    Fetch(#[from] FetchError),
    #[error("Internal error while compiling: {0}")]
    Internal(#[from] SolcError),
    #[error("Compilation error: {0:?}")]
    Compilation(Vec<String>),
    #[error("failed to acquire lock: {0}")]
    Acquire(#[from] AcquireError),
}

#[async_trait::async_trait]
pub trait EvmCompiler {
    async fn compile(
        &self,
        path: &Path,
        ver: &Version,
        input: &CompilerInput,
    ) -> Result<CompilerOutput, SolcError>;
}

pub struct Compilers<C> {
    cache: DownloadCache,
    fetcher: Arc<dyn Fetcher>,
    evm_compiler: C,
    threads_semaphore: Arc<Semaphore>,
}

impl<C> Compilers<C>
where
    C: EvmCompiler,
{
    pub fn new(
        fetcher: Arc<dyn Fetcher>,
        evm_compiler: C,
        threads_semaphore: Arc<Semaphore>,
    ) -> Self {
        Self {
            cache: DownloadCache::new(),
            fetcher,
            evm_compiler,
            threads_semaphore,
        }
    }
    #[instrument(name = "download_and_compile", skip(self, input), level = "debug")]
    pub async fn compile(
        &self,
        compiler_version: &Version,
        input: &CompilerInput,
    ) -> Result<CompilerOutput, Error> {
        let path_result = {
            self.cache
                .get(self.fetcher.as_ref(), compiler_version)
                .await
        };
        let path = match path_result {
            Err(FetchError::NotFound(version)) => return Err(Error::VersionNotFound(version)),
            res => res?,
        };

        let output = {
            let span = tracing::debug_span!(
                "compile contract with ethers-solc",
                ver = compiler_version.to_string()
            );
            let _span_guard = span.enter();
            let _permit = {
                let _wait_timer_guard = metrics::COMPILATION_QUEUE_TIME.start_timer();
                let _wait_gauge_guard = metrics::COMPILATIONS_IN_QUEUE.guarded_inc();
                self.threads_semaphore.acquire().await?
            };
            let _compile_timer_guard = metrics::COMPILE_TIME.start_timer();
            let _compile_gauge_guard = metrics::COMPILATIONS_IN_FLIGHT.guarded_inc();
            self.evm_compiler
                .compile(&path, compiler_version, input)
                .await?
        };

        // Compilations errors, warnings and info messages are returned in `CompilerOutput.error`
        let mut errors = Vec::new();
        for err in &output.errors {
            if err.severity == Severity::Error {
                errors.push(
                    err.formatted_message
                        .as_ref()
                        .unwrap_or(&err.message)
                        .clone(),
                )
            }
        }
        if !errors.is_empty() {
            return Err(Error::Compilation(errors));
        }

        Ok(output)
    }

    pub fn all_versions(&self) -> Vec<Version> {
        self.fetcher.all_versions()
    }

    pub fn all_versions_sorted_str(&self) -> Vec<String> {
        let mut versions = self.all_versions();
        // sort in descending order
        versions.sort_by(|x, y| x.cmp(y).reverse());
        versions.into_iter().map(|v| v.to_string()).collect()
    }

    pub async fn load_from_dir(&self, dir: &PathBuf) {
        match self.cache.load_from_dir(dir).await {
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(
                    "cannot load local compilers from `{}` dir: {}",
                    dir.to_string_lossy(),
                    e
                )
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::{super::list_fetcher::ListFetcher, *};
    use crate::{consts::DEFAULT_SOLIDITY_COMPILER_LIST, solidity::SolidityCompiler};
    use ethers_solc::artifacts::{Source, Sources};
    use std::{default::Default, env::temp_dir, str::FromStr};
    use tokio::sync::{OnceCell, Semaphore};

    async fn global_compilers() -> &'static Compilers<SolidityCompiler> {
        static COMPILERS: OnceCell<Compilers<SolidityCompiler>> = OnceCell::const_new();
        COMPILERS
            .get_or_init(|| async {
                let url = DEFAULT_SOLIDITY_COMPILER_LIST
                    .try_into()
                    .expect("Getting url");
                let fetcher = ListFetcher::new(url, temp_dir(), None, None)
                    .await
                    .expect("Fetch releases");
                let threads_semaphore = Arc::new(Semaphore::new(4));
                Compilers::new(
                    Arc::new(fetcher),
                    SolidityCompiler::new(),
                    threads_semaphore,
                )
            })
            .await
    }

    struct Input {
        source_code: String,
    }

    impl Input {
        pub fn with_source_code(source_code: String) -> Self {
            Self { source_code }
        }
    }

    impl From<Input> for CompilerInput {
        fn from(input: Input) -> Self {
            let mut compiler_input = CompilerInput {
                language: "Solidity".to_string(),
                sources: Sources::from([("source.sol".into(), Source::new(input.source_code))]),
                settings: Default::default(),
            };
            compiler_input.settings.evm_version = None;
            compiler_input
        }
    }

    #[tokio::test]
    async fn successful_compilation() {
        let source_code = r#"
            pragma solidity ^0.8.10;
            // SPDX-License-Identifier: MIT

            contract SimpleStorage {
                uint storedData;

                function set(uint x) public {
                    storedData = x;
                }

                function get() public view returns (uint) {
                    return storedData;
                }
            }"#;

        let compilers = global_compilers().await;
        let input: CompilerInput = Input::with_source_code(source_code.into()).into();
        let version = Version::from_str("v0.8.10+commit.fc410830").expect("Compiler version");

        let result = compilers
            .compile(&version, &input)
            .await
            .expect("Compilation failed");
        assert!(
            !result.contracts.is_empty(),
            "Result should consists of at least one contract"
        );
    }

    #[tokio::test]
    async fn can_compile_large_file() {
        let source_code = include_str!("test_data/large_smart_contract.sol");

        let compilers = global_compilers().await;
        let input: CompilerInput = Input::with_source_code(source_code.into()).into();
        let version = Version::from_str("v0.5.9+commit.c68bc34e").expect("Compiler version");

        let result = compilers
            .compile(&version, &input)
            .await
            .expect("Compilation failed");
        assert!(
            !result.contracts.is_empty(),
            "Result should consists of at least one contract"
        );
    }

    #[tokio::test]
    async fn returns_compilation_error() {
        let source_code = r#"pragma solidity ^0.8.10; cont SimpleStorage {"#;

        let compilers = global_compilers().await;
        let input: CompilerInput = Input::with_source_code(source_code.into()).into();
        let version = Version::from_str("v0.8.10+commit.fc410830").expect("Compiler version");

        let result = compilers
            .compile(&version, &input)
            .await
            .expect_err("Compilation should fail");
        match result {
            Error::Compilation(errors) => {
                assert!(errors.into_iter().any(|err| err.contains("ParserError")))
            }
            _ => panic!("Invalid compilation error: {result:?}"),
        }
    }
}
