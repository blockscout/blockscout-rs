use super::solc_cli;
use crate::compiler::{EvmCompiler, Version};
use ethers_solc::{error::SolcError, CompilerOutput, Solc};
use std::path::Path;

#[derive(Default)]
pub struct SolidityCompiler {}

impl SolidityCompiler {
    pub fn new() -> Self {
        SolidityCompiler {}
    }
}

#[async_trait::async_trait]
impl EvmCompiler for SolidityCompiler {
    type CompilerInput = ethers_solc::CompilerInput;

    async fn compile(
        &self,
        path: &Path,
        ver: &Version,
        input: &Self::CompilerInput,
    ) -> Result<CompilerOutput, SolcError> {
        if ver.version() < &semver::Version::new(0, 4, 11) {
            solc_cli::compile_using_cli(path, input).await
        } else {
            Solc::from(path).async_compile(input).await
        }
    }
}
