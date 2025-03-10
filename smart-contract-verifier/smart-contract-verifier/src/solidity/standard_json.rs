use super::client::Client;
use crate::{
    batch_verifier::BatchError, compiler::DetailedVersion, BatchVerificationResult, Contract,
    OnChainCode,
};
use bytes::Bytes;
use foundry_compilers::CompilerInput;
use std::sync::Arc;

pub use standard_json_new::{verify, StandardJsonParseError, VerificationRequestNew};
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
        pub contract_address: Option<alloy_core::primitives::Address>,
    }

    pub async fn verify(
        client: Arc<Client>,
        request: VerificationRequestNew,
    ) -> Result<verify_new::VerificationResult, verify_new::Error> {
        let to_verify = vec![verify_new::OnChainContract {
            on_chain_code: request.on_chain_code,
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

    pub use proto::StandardJsonParseError;
    mod proto {
        use super::*;
        use crate::verify_new::SolcInput;
        use anyhow::Context;
        use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
            BytecodeType, VerifySolidityStandardJsonRequest,
        };
        use std::str::FromStr;

        #[derive(thiserror::Error, Debug)]
        pub enum StandardJsonParseError {
            #[error("content is not a valid standard json: {0}")]
            InvalidContent(#[from] serde_path_to_error::Error<serde_json::Error>),
            #[error("{0:#}")]
            BadRequest(#[from] anyhow::Error),
        }

        impl TryFrom<VerifySolidityStandardJsonRequest> for VerificationRequestNew {
            type Error = StandardJsonParseError;

            fn try_from(request: VerifySolidityStandardJsonRequest) -> Result<Self, Self::Error> {
                let code_value = blockscout_display_bytes::decode_hex(&request.bytecode)
                    .context("bytecode is not valid hex")?;
                let on_chain_code = match request.bytecode_type() {
                    BytecodeType::Unspecified => {
                        Err(anyhow::anyhow!("bytecode type is unspecified"))?
                    }
                    BytecodeType::CreationInput => OnChainCode::creation(code_value),
                    BytecodeType::DeployedBytecode => OnChainCode::runtime(code_value),
                };

                let compiler_version = DetailedVersion::from_str(&request.compiler_version)
                    .context("invalid compiler version")?;

                let deserializer = &mut serde_json::Deserializer::from_str(&request.input);
                let input: SolcInput = serde_path_to_error::deserialize(deserializer)?;

                let (chain_id, contract_address) = match request.metadata {
                    None => (None, None),
                    Some(metadata) => {
                        let chain_id = metadata.chain_id;
                        let contract_address = metadata
                            .contract_address
                            .map(|value| alloy_core::primitives::Address::from_str(&value))
                            .transpose()
                            .ok()
                            .flatten();
                        (chain_id, contract_address)
                    }
                };

                Ok(Self {
                    on_chain_code,
                    compiler_version,
                    content: input,
                    chain_id,
                    contract_address,
                })
            }
        }
    }
}

pub struct VerificationRequest {
    pub deployed_bytecode: Bytes,
    pub creation_bytecode: Option<Bytes>,
    pub compiler_version: DetailedVersion,

    pub content: StandardJsonContent,

    // Required for the metrics. Has no functional meaning.
    // In case if chain_id has not been provided, results in empty string.
    pub chain_id: Option<String>,
}

pub struct StandardJsonContent {
    pub input: CompilerInput,
}

impl From<StandardJsonContent> for CompilerInput {
    fn from(content: StandardJsonContent) -> Self {
        content.input
    }
}

pub struct BatchVerificationRequest {
    pub contracts: Vec<Contract>,
    pub compiler_version: DetailedVersion,
    pub content: StandardJsonContent,
}

pub async fn batch_verify(
    client: Arc<Client>,
    request: BatchVerificationRequest,
) -> Result<Vec<BatchVerificationResult>, BatchError> {
    let compiler_input = CompilerInput::from(request.content);

    let verification_result = crate::batch_verifier::verify_solidity(
        client.compilers(),
        request.compiler_version,
        request.contracts,
        &compiler_input,
    )
    .await?;

    Ok(verification_result)
}
