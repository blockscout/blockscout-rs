use super::VerificationRequest;
use crate::batch_verifier::zksync_batch_contract_verifier;
use crate::compiler::ZkSyncCompilers;
use crate::{BatchError, compiler::ZkSyncCompiler, Contract, ZkSolcCompiler};
use crate::batch_verifier::zksync_batch_contract_verifier::VerificationResult;

pub type Content = <ZkSolcCompiler as ZkSyncCompiler>::CompilerInput;

pub async fn verify(
    compilers: &ZkSyncCompilers<ZkSolcCompiler>,
    request: VerificationRequest<Content>,
) -> Result<VerificationResult, BatchError> {
    let (creation_code, runtime_code) =
        if let Some(constructor_arguments) = request.constructor_arguments {
            let creation_code = request
                .code
                .to_vec()
                .into_iter()
                .chain(constructor_arguments.to_vec())
                .collect::<Vec<_>>()
                .into();
            (Some(creation_code), Some(request.code.into()))
        } else {
            (None, Some(request.code.into()))
        };
    let contracts = vec![Contract {
        creation_code,
        runtime_code,
    }];

    let mut verification_result = zksync_batch_contract_verifier::verify_zksolc(
        compilers,
        request.zk_compiler,
        request.solc_compiler,
        contracts,
        &request.content,
    )
    .await?;

    Ok(verification_result.pop().expect("batch with exactly one element must be returned"))
}
