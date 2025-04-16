use crate::{
    compiler::DetailedVersion, verify_new, verify_new::VyperCompiler, EvmCompilersPool,
    OnChainContract,
};

pub type Content = verify_new::VyperInput;

#[derive(Clone, Debug)]
pub struct VerificationRequest {
    pub contract: OnChainContract,
    pub compiler_version: DetailedVersion,
    pub content: Content,
}

pub async fn verify(
    compilers: &EvmCompilersPool<VyperCompiler>,
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
