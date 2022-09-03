use std::path::Path;

use ethers_solc::{error::SolcError, CompilerOutput, Solc};

use crate::compilers::{EvmCompiler, Version};

use super::solc_cli;

pub struct SolidityCompiler {}

impl SolidityCompiler {
    pub fn new() -> Self {
        SolidityCompiler {}
    }
}

#[async_trait::async_trait]
impl EvmCompiler for SolidityCompiler {
    async fn compile(
        &self,
        path: &Path,
        ver: &Version,
        input: &ethers_solc::CompilerInput,
    ) -> Result<CompilerOutput, SolcError> {
        if ver.version() < &semver::Version::new(0, 4, 11) {
            solc_cli::compile_using_cli(path, input).await
        } else {
            Solc::from(path).compile(input)
        }
    }
}
