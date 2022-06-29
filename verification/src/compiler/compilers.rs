use crate::compiler::{CompilerVersion, DownloadCache, Fetcher, VersionList};
use anyhow::anyhow;
use ethers_solc::{
    artifacts::{self, Severity},
    error::SolcError,
    CompilerInput, CompilerOutput, Solc,
};
use std::fmt::{Debug, Display};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompilersError {
    #[error("Error while fetching compiler: {0:#}")]
    Fetch(anyhow::Error),
    #[error("Internal error while compiling: {0}")]
    Internal(#[from] SolcError),
    #[error("Compilation error: {0:?}")]
    Compilation(Vec<artifacts::Error>),
}

pub struct Compilers<T> {
    cache: DownloadCache,
    fetcher: T,
}

impl<T: Fetcher> Compilers<T> {
    pub fn new(fetcher: T) -> Self {
        Self {
            cache: DownloadCache::new(),
            fetcher,
        }
    }

    pub async fn compile(
        &self,
        compiler_version: &CompilerVersion,
        input: &CompilerInput,
    ) -> Result<CompilerOutput, CompilersError>
    where
        <T as Fetcher>::Error: Debug + Display,
    {
        let solc_path = self
            .cache
            .get(&self.fetcher, compiler_version)
            .await
            .map_err(|err| CompilersError::Fetch(anyhow!(err)))?;
        let solc = Solc::from(solc_path);
        let output = solc.compile(&input)?;

        // Compilations errors, warnings and info messages are returned in `CompilerOutput.error`
        let mut errors = Vec::new();
        for err in &output.errors {
            if err.severity == Severity::Error {
                errors.push(err.clone())
            }
        }
        if !errors.is_empty() {
            return Err(CompilersError::Compilation(errors));
        }

        Ok(output)
    }
}

impl<T: VersionList> VersionList for Compilers<T> {
    fn all_versions(&self) -> Vec<&CompilerVersion> {
        self.fetcher.all_versions()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solidity::{CompilerFetcher, Releases};
    use std::{env::temp_dir, str::FromStr};

    use crate::consts::DEFAULT_COMPILER_LIST;
    use async_once_cell::OnceCell;
    use ethers_solc::artifacts::{Source, Sources};
    use std::default::Default;

    async fn global_compilers() -> &'static Compilers<CompilerFetcher> {
        static COMPILERS: OnceCell<Compilers<CompilerFetcher>> = OnceCell::new();
        COMPILERS
            .get_or_init(async {
                let url = DEFAULT_COMPILER_LIST.try_into().expect("Getting url");
                let releases = Releases::fetch_from_url(&url)
                    .await
                    .expect("Fetch releases");
                let fetcher = CompilerFetcher::new(releases, temp_dir()).await;
                let compilers = Compilers::new(fetcher);
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
            CompilerVersion::from_str("v0.8.10+commit.fc410830").expect("Compiler version");

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
            CompilerVersion::from_str("v0.5.9+commit.c68bc34e").expect("Compiler version");

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
            CompilerVersion::from_str("v0.8.10+commit.fc410830").expect("Compiler version");

        let result = compilers
            .compile(&version, &input)
            .await
            .expect_err("Compilation should fail");
        match result {
            CompilersError::Compilation(errors) => {
                assert!(errors.into_iter().any(|err| err.r#type == "ParserError"))
            }
            _ => panic!("Invalid compilation error: {:?}", result),
        }
    }
}
