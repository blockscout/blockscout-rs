use crate::compiler::{EvmCompiler, ZkSyncCompiler, ZkError};
use crate::zksolc_standard_json::input::Input as ZkStandardJsonCompilerInput;
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;
use foundry_compilers::error::SolcError;

#[derive(Default)]
pub struct ZkSolcCompiler {}

#[async_trait]
impl ZkSyncCompiler for ZkSolcCompiler {
    type CompilerInput = ZkStandardJsonCompilerInput;

    async fn compile(
        zk_compiler_path: &Path,
        evm_compiler_path: &Path,
        input: &Self::CompilerInput,
    ) -> Result<Value, SolcError> {
        let raw = foundry_compilers::Solc::new(zk_compiler_path)
            .arg(format!("--solc={}", evm_compiler_path.to_string_lossy()))
            .compile_output(input)?;

        serde_json::from_slice(&raw).map_err(|err| err.into())
    }
}
