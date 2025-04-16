use crate::{
    compiler::DetailedVersion, verify_new, verify_new::SolcCompiler, EvmCompilersPool,
    OnChainContract,
};

type Content = verify_new::SolcInput;

pub struct VerificationRequest {
    pub contract: OnChainContract,
    pub compiler_version: DetailedVersion,
    pub content: Content,
}

pub async fn verify(
    compilers: &EvmCompilersPool<SolcCompiler>,
    request: VerificationRequest,
) -> Result<verify_new::VerificationResult, verify_new::Error> {
    let to_verify = vec![request.contract];

    let results = verify_new::compile_and_verify(
        to_verify,
        compilers,
        &request.compiler_version,
        request.content,
    )
    .await?;
    let result = results
        .into_iter()
        .next()
        .expect("we sent exactly one contract to verify");

    Ok(result)
}

#[derive(Clone, Debug)]
pub struct BatchVerificationRequest {
    pub contracts: Vec<OnChainContract>,
    pub compiler_version: DetailedVersion,
    pub content: Content,
}

pub async fn batch_verify(
    compilers: &EvmCompilersPool<SolcCompiler>,
    request: BatchVerificationRequest,
) -> Result<Vec<verify_new::VerificationResult>, verify_new::Error> {
    let to_verify = request.contracts;

    let results = verify_new::compile_and_verify(
        to_verify,
        compilers,
        &request.compiler_version,
        request.content,
    )
    .await?;

    Ok(results)
}
