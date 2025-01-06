use super::{artifacts::CompilerInput, client::Client, types::Success};
use crate::{
    compiler::DetailedVersion,
    verifier::{ContractVerifier, Error},
};
use bytes::Bytes;
use std::sync::Arc;

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
