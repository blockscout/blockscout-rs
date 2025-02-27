use super::{client::Client, types::Success};
use crate::{
    batch_verifier::BatchError,
    compiler::DetailedVersion,
    verifier::{ContractVerifier, Error},
    verifier_new::OnChainCode,
    BatchVerificationResult, Contract,
};
use bytes::Bytes;
use foundry_compilers::CompilerInput;
use std::sync::Arc;

pub struct VerificationRequestNew {
    pub on_chain_code: OnChainCode,
    pub compiler_version: DetailedVersion,
    pub content: StandardJsonContentNew,

    // metadata
    pub chain_id: Option<String>,
    pub contract_address: Option<alloy_core::primitives::Address>,
}

pub struct StandardJsonContentNew {
    pub input: foundry_compilers_new::artifacts::SolcInput,
}

impl From<StandardJsonContentNew> for foundry_compilers_new::artifacts::SolcInput {
    fn from(content: StandardJsonContentNew) -> foundry_compilers_new::artifacts::SolcInput {
        content.input
    }
}

pub async fn verify_new(
    client: Arc<Client>,
    request: VerificationRequestNew,
) -> Result<Success, Error> {
    todo!()
}

pub use proto::StandardJsonParseError;
mod proto {
    use super::*;
    use anyhow::Context;
    use foundry_compilers_new::artifacts::SolcInput;
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
                BytecodeType::Unspecified => Err(anyhow::anyhow!("bytecode type is unspecified"))?,
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
                content: StandardJsonContentNew { input },
                chain_id,
                contract_address,
            })
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

pub async fn verify(client: Arc<Client>, request: VerificationRequest) -> Result<Success, Error> {
    let compiler_input = CompilerInput::from(request.content);
    let verifier = ContractVerifier::new(
        false,
        client.compilers(),
        &request.compiler_version,
        request.creation_bytecode,
        request.deployed_bytecode,
        request.chain_id,
    )?;
    let result = verifier.verify(&compiler_input).await?;

    // If case of success, we allow middlewares to process success and only then return it to the caller
    let success = Success::from((compiler_input, result));
    if let Some(middleware) = client.middleware() {
        middleware.call(&success).await;
    }

    Ok(success)
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
