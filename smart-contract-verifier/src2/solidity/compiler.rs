use std::path::Path;

use ethers_solc::{error::SolcError, CompilerOutput, Solc};

use crate::compiler::EvmCompiler;

use super::compile_using_cli;

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
        ver: &crate::compiler::Version,
        input: &ethers_solc::CompilerInput,
    ) -> Result<CompilerOutput, SolcError> {
        if ver.version() < &semver::Version::new(0, 4, 11) {
            compile_using_cli(path, input).await
        } else {
            Solc::from(path).compile(input)
        }
    }
}
