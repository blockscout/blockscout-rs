use std::collections::BTreeMap;
use crate::common_types::Contract;
use crate::compiler::ZkSyncCompiler;
use crate::{BatchSuccess, CompactVersion, DetailedVersion, MatchType, ZkSolcCompiler, ZkSyncCompilers};
use crate::batch_verifier::{BatchError, compilation, zk_compilation};
use crate::verifier::CompilerInput;

pub type VerificationResult = crate::batch_verifier::VerificationResult<ZkBatchSuccess>;

#[derive(Clone, Debug, Default)]
pub struct ZkBatchSuccess {
    pub zk_compiler: String,
    pub zk_compiler_version: String,
    pub batch_success: BatchSuccess,
}

pub async fn verify_zksolc(
    compilers: &ZkSyncCompilers<ZkSolcCompiler>,
    zk_compiler_version: CompactVersion,
    evm_compiler_version: DetailedVersion,
    contracts: Vec<Contract>,
    compiler_input: &<ZkSolcCompiler as ZkSyncCompiler>::CompilerInput,
) -> Result<Vec<VerificationResult>, BatchError> {
    let compiler_output = compilers
        .compile(&zk_compiler_version, &evm_compiler_version, compiler_input)
        .await?;

    let modified_compiler_output = {
        let compiler_input = compiler_input.clone().modify();
        compilers
            .compile(&zk_compiler_version, &evm_compiler_version, &compiler_input)
            .await?
    };

    let compilation_result = zk_compilation::parse_zksolc_contracts(
        zk_compiler_version,
        evm_compiler_version,
        compiler_input,
        compiler_output,
        modified_compiler_output,
    )
        .map_err(|err| {
            tracing::error!("parsing compiled contracts failed: {err:#}");
            BatchError::Internal(err)
        })?;


    todo!()
}
