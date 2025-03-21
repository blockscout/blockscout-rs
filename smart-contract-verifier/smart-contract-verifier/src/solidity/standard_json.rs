use super::client::Client;
use crate::{compiler::DetailedVersion, verify_new, OnChainCode};
use foundry_compilers::CompilerInput;
use std::sync::Arc;

use crate::{verify_new::SolcInput, OnChainContract};
pub use standard_json_new::{verify, VerificationRequestNew};

mod standard_json_new {
    use super::*;
    use crate::verify_new;
    use verify_new::SolcInput;

    pub struct VerificationRequestNew {
        pub on_chain_code: OnChainCode,
        pub compiler_version: DetailedVersion,
        pub content: SolcInput,

        // metadata
        pub chain_id: Option<String>,
        pub address: Option<alloy_core::primitives::Address>,
    }

    pub async fn verify(
        client: Arc<Client>,
        request: VerificationRequestNew,
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
}

pub struct StandardJsonContent {
    pub input: CompilerInput,
}

impl From<StandardJsonContent> for CompilerInput {
    fn from(content: StandardJsonContent) -> Self {
        content.input
    }
}

#[derive(Clone, Debug)]
pub struct BatchVerificationRequestNew {
    pub contracts: Vec<OnChainContract>,
    pub compiler_version: DetailedVersion,
    pub content: SolcInput,
}

pub async fn batch_verify(
    client: Arc<Client>,
    request: BatchVerificationRequestNew,
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
