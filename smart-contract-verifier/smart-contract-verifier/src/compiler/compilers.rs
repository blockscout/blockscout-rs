use super::{
    download_cache::DownloadCache,
    fetcher::{FetchError, Fetcher},
    version_detailed::DetailedVersion,
};
use crate::metrics::{self, GuardedGauge};
use ethers_solc::{artifacts::Severity, error::SolcError, CompilerOutput};
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
    VersionNotFound(String),
    #[error("Error while fetching compiler: {0:#}")]
    Fetch(#[from] FetchError),
    #[error("Internal error while compiling: {0}")]
    Internal(#[from] SolcError),
    #[error("Compilation error: {0:?}")]
    Compilation(Vec<String>),
    #[error("failed to acquire lock: {0}")]
    Acquire(#[from] AcquireError),
}

pub trait CompilerInput {
    /// Modifies input so that the corresponding bytecode
    /// should have modified metadata hash, if any.
    fn modify(self) -> Self;

    fn normalize_output_selection(&mut self, version: &DetailedVersion);
}

#[async_trait::async_trait]
pub trait EvmCompiler {
    type CompilerInput: CompilerInput + Clone;

    async fn compile(
        &self,
        path: &Path,
        ver: &DetailedVersion,
        input: &Self::CompilerInput,
    ) -> Result<(serde_json::Value, CompilerOutput), SolcError>;
}

pub struct Compilers<C> {
    cache: DownloadCache<DetailedVersion>,
    fetcher: Arc<dyn Fetcher<Version = DetailedVersion>>,
    evm_compiler: C,
    threads_semaphore: Arc<Semaphore>,
}

impl<C> Compilers<C>
where
    C: EvmCompiler,
{
    pub fn new(
        fetcher: Arc<dyn Fetcher<Version = DetailedVersion>>,
        evm_compiler: C,
        threads_semaphore: Arc<Semaphore>,
    ) -> Self {
        Self {
            cache: Default::default(),
            fetcher,
            evm_compiler,
            threads_semaphore,
        }
    }
    #[instrument(name = "download_and_compile", skip(self, input), level = "debug")]
    pub async fn compile(
        &self,
        compiler_version: &DetailedVersion,
        input: &C::CompilerInput,
        chain_id: Option<&str>,
    ) -> Result<(serde_json::Value, CompilerOutput), Error> {
        let mut input = input.clone();
        input.normalize_output_selection(compiler_version);
        let path_result = {
            self.cache
                .get(self.fetcher.as_ref(), compiler_version)
                .await
        };
        let path = match path_result {
            Err(FetchError::NotFound(version)) => return Err(Error::VersionNotFound(version)),
            res => res?,
        };

        let (raw, output) = {
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
            let _compile_timer_guard = metrics::COMPILE_TIME
                .with_label_values(&[chain_id.unwrap_or_default()])
                .start_timer();
            let _compile_gauge_guard = metrics::COMPILATIONS_IN_FLIGHT.guarded_inc();
            self.evm_compiler
                .compile(&path, compiler_version, &input)
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

        Ok((raw, output))
    }

    pub fn all_versions(&self) -> Vec<DetailedVersion> {
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
    use super::{super::ListFetcher, *};
    use crate::{consts::DEFAULT_SOLIDITY_COMPILER_LIST, solidity::SolidityCompiler};
    use foundry_compilers::{
        artifacts::{Source, Sources},
        CompilerInput,
    };
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
        let version =
            DetailedVersion::from_str("v0.8.10+commit.fc410830").expect("Compiler version");

        let (_raw, result) = compilers
            .compile(&version, &input, None)
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
        let version =
            DetailedVersion::from_str("v0.5.9+commit.c68bc34e").expect("Compiler version");

        let (_raw, result) = compilers
            .compile(&version, &input, None)
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
        let version =
            DetailedVersion::from_str("v0.8.10+commit.fc410830").expect("Compiler version");

        let result = compilers
            .compile(&version, &input, None)
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
