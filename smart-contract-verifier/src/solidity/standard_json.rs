use crate::{
    compilers::{Compilers, Version},
    solidity::{
        compiler::SolidityCompiler,
        contract_verifier::{ContractVerifier, Error},
        verifier::VerificationSuccess,
    },
};
use bytes::Bytes;
use ethers_solc::{artifacts::output_selection::OutputSelection, CompilerInput};

pub struct VerificationRequest {
    pub deployed_bytecode: Bytes,
    pub creation_bytecode: Bytes,
    pub compiler_version: Version,

    pub content: StandardJsonContent,
}

pub struct StandardJsonContent {
    input: CompilerInput,
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

pub async fn verify(
    compilers: Compilers<SolidityCompiler>,
    request: VerificationRequest,
) -> Result<VerificationSuccess, Error> {
    let compiler_input = CompilerInput::from(request.content);
    let verifier = ContractVerifier::new(
        compilers,
        &request.compiler_version,
        request.creation_bytecode,
        request.deployed_bytecode,
    )?;
    verifier.verify(&compiler_input).await
}
