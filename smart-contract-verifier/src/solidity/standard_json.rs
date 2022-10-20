use super::client::Client;
use crate::{
    compiler::Version,
    verifier::{ContractVerifier, Error, Success},
};
use bytes::Bytes;
use ethers_solc::{artifacts::output_selection::OutputSelection, CompilerInput};
use std::sync::Arc;

pub struct VerificationRequest {
    pub deployed_bytecode: Bytes,
    pub creation_bytecode: Option<Bytes>,
    pub compiler_version: Version,

    pub content: StandardJsonContent,
}

pub struct StandardJsonContent {
    pub input: CompilerInput,
}

impl From<StandardJsonContent> for CompilerInput {
    fn from(content: StandardJsonContent) -> Self {
        let mut input = content.input;

        // always overwrite output selection as it customizes what compiler outputs and
        // is not what is returned to the user, but only used internally by our service
        let output_selection = OutputSelection::default_output_selection();
        input.settings.output_selection = output_selection;

        input
    }
}

pub async fn verify(client: Arc<Client>, request: VerificationRequest) -> Result<Success, Error> {
    let compiler_input = CompilerInput::from(request.content);
    let verifier = ContractVerifier::new(
        client.compilers(),
        &request.compiler_version,
        request.creation_bytecode,
        request.deployed_bytecode,
    )?;
    let result = verifier.verify(&compiler_input).await;

    // If case of success, we allow middlewares to process success and only then return it to the caller
    let success = result?;
    if let Some(middleware) = client.middleware() {
        middleware.call(&success).await;
    }
    Ok(success)
}
