use super::fetcher::FetchError;
use crate::compiler::{self, DownloadCache, Fetcher};
use ethers_solc::{artifacts::Severity, error::SolcError, CompilerInput, CompilerOutput, Solc};
use std::{fmt::Debug, sync::Arc};
use thiserror::Error as DeriveError;

#[derive(Debug, DeriveError)]
pub enum Error {
    #[error("Error while fetching compiler: {0:#}")]
    Fetch(#[from] FetchError),
    #[error("Internal error while compiling: {0}")]
    Internal(#[from] SolcError),
    #[error("Compilation error: {0:?}")]
    Compilation(Vec<String>),
}

pub struct Compilers {
    cache: DownloadCache,
    fetcher: Arc<dyn Fetcher>,
}

impl Compilers {
    pub fn new(fetcher: Arc<dyn Fetcher>) -> Self {
        Self {
            cache: DownloadCache::new(),
            fetcher,
        }
    }

    pub async fn compile(
        &self,
        compiler_version: &compiler::Version,
        input: &CompilerInput,
    ) -> Result<CompilerOutput, Error> {
        let solc_path = self.cache.get(&*self.fetcher, compiler_version).await?;
        let solc = Solc::from(solc_path);
        let output = solc.compile(&input)?;

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

    pub fn all_versions(&self) -> Vec<compiler::Version> {
        self.fetcher.all_versions()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::ListFetcher;
    use std::{env::temp_dir, str::FromStr};

    use crate::consts::DEFAULT_COMPILER_LIST;
    use async_once_cell::OnceCell;
    use ethers_solc::artifacts::{Source, Sources};
    use std::default::Default;

    async fn global_compilers() -> &'static Compilers {
        static COMPILERS: OnceCell<Compilers> = OnceCell::new();
        COMPILERS
            .get_or_init(async {
                let url = DEFAULT_COMPILER_LIST.try_into().expect("Getting url");
                let fetcher = ListFetcher::new(url, None, temp_dir())
                    .await
                    .expect("Fetch releases");
                let compilers = Compilers::new(Arc::new(fetcher));
                compilers
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
                sources: Sources::from([(
                    "source.sol".into(),
                    Source {
                        content: input.source_code,
                    },
                )]),
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
            compiler::Version::from_str("v0.8.10+commit.fc410830").expect("Compiler version");

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
        let version =
            compiler::Version::from_str("v0.5.9+commit.c68bc34e").expect("Compiler version");

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
        let version =
            compiler::Version::from_str("v0.8.10+commit.fc410830").expect("Compiler version");

        let result = compilers
            .compile(&version, &input)
            .await
            .expect_err("Compilation should fail");
        match result {
            Error::Compilation(errors) => {
                assert!(errors.into_iter().any(|err| err.contains("ParserError")))
            }
            _ => panic!("Invalid compilation error: {:?}", result),
        }
    }
}
