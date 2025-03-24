use super::client::Client;
use crate::{compiler::DetailedVersion, verify_new, OnChainCode, OnChainContract};
use std::sync::Arc;

type Content = verify_new::SolcInput;

pub struct VerificationRequest {
    pub on_chain_code: OnChainCode,
    pub compiler_version: DetailedVersion,
    pub content: Content,

    // metadata
    pub chain_id: Option<String>,
    pub address: Option<alloy_core::primitives::Address>,
}

pub async fn verify(
    client: Arc<Client>,
    request: VerificationRequest,
) -> Result<verify_new::VerificationResult, verify_new::Error> {
    let to_verify = vec![OnChainContract {
        code: request.on_chain_code,
        chain_id: request.chain_id,
        address: request.address,
    }];
    let compilers = client.new_compilers();

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
    client: Arc<Client>,
    request: BatchVerificationRequest,
) -> Result<Vec<verify_new::VerificationResult>, verify_new::Error> {
    let to_verify = request.contracts;
    let compilers = client.new_compilers();

    let results = verify_new::compile_and_verify(
        to_verify,
        compilers,
        &request.compiler_version,
        request.content,
    )
    .await?;

    Ok(results)
}
