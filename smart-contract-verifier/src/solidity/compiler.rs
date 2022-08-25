use std::path::Path;

use ethers_solc::{error::SolcError, CompilerOutput, Solc};

use crate::compiler::EvmCompilerAgent;

pub struct SolidityCompilerAgent {}
impl SolidityCompilerAgent {
    pub fn new() -> Self {
        SolidityCompilerAgent {}
    }
}
impl EvmCompilerAgent for SolidityCompilerAgent {
    fn compile(
        &self,
        path: &Path,
        ver: &crate::compiler::Version,
        input: &ethers_solc::CompilerInput,
    ) -> Result<CompilerOutput, SolcError> {
        let _ = ver;
        Solc::from(path).compile(input)
    }
}
