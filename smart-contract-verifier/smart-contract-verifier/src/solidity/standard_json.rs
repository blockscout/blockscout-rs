use super::{client::Client, types::Success};
use crate::{
    compiler::Version,
    verifier::{ContractVerifier, Error},
};
use bytes::Bytes;
use ethers_solc::{artifacts::output_selection::OutputSelection, CompilerInput};
use std::sync::Arc;

pub struct VerificationRequest {
    pub deployed_bytecode: Bytes,
    pub creation_bytecode: Option<Bytes>,
    pub compiler_version: Version,

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
        let mut input = content.input;

        // always overwrite output selection as it customizes what compiler outputs and
        // is not what is returned to the user, but only used internally by our service
        let output_selection = OutputSelection::complete_output_selection();
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

pub mod proto {
    use super::{StandardJsonContent, VerificationRequest};
    use crate::Version;
    use blockscout_display_bytes::Bytes as DisplayBytes;
    use conversion_primitives::InvalidArgument;
    use ethers_solc::CompilerInput;
    use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
        BytecodeType, VerifySolidityStandardJsonRequest,
    };
    use std::str::FromStr;
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum StandardJsonParseError {
        #[error("content is not a valid standard json: {0}")]
        InvalidContent(#[from] serde_json::Error),
        #[error(transparent)]
        InvalidArgument(#[from] InvalidArgument),
    }

    impl TryFrom<VerifySolidityStandardJsonRequest> for VerificationRequest {
        type Error = StandardJsonParseError;

        fn try_from(request: VerifySolidityStandardJsonRequest) -> Result<Self, Self::Error> {
            let bytecode = DisplayBytes::from_str(&request.bytecode)
                .map_err(|err| {
                    InvalidArgument::new(format!("Invalid deployed bytecode: {:?}", err))
                })?
                .0;
            let (creation_bytecode, deployed_bytecode) = match request.bytecode_type() {
                BytecodeType::Unspecified => {
                    Err(InvalidArgument::new("Bytecode type is unspecified"))?
                }
                BytecodeType::CreationInput => (Some(bytecode), bytes::Bytes::new()),
                BytecodeType::DeployedBytecode => (None, bytecode),
            };
            let compiler_version = Version::from_str(&request.compiler_version).map_err(|err| {
                InvalidArgument::new(format!("Invalid compiler version: {}", err))
            })?;

            let input: CompilerInput = serde_json::from_str(&request.input)?;

            Ok(Self {
                deployed_bytecode,
                creation_bytecode,
                compiler_version,
                content: StandardJsonContent { input },
                chain_id: request.metadata.and_then(|metadata| metadata.chain_id),
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use pretty_assertions::assert_eq;
        use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::VerificationMetadata;

        #[test]
        fn try_into_verification_request() {
            /********** Creation Input **********/

            let mut request = VerifySolidityStandardJsonRequest {
                bytecode: "0x1234".to_string(),
                bytecode_type: BytecodeType::CreationInput.into(),
                compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
                input: "{\"language\": \"Solidity\", \"sources\": {\"./src/contracts/Foo.sol\": {\"content\": \"pragma solidity ^0.8.2;\\n\\ncontract Foo {\\n    function bar() external pure returns (uint256) {\\n        return 42;\\n    }\\n}\\n\"}}, \"settings\": {\"metadata\": {\"useLiteralContent\": true}, \"optimizer\": {\"enabled\": true, \"runs\": 200}, \"outputSelection\": {\"*\": {\"*\": [\"abi\", \"evm.bytecode\", \"evm.deployedBytecode\", \"evm.methodIdentifiers\"], \"\": [\"id\", \"ast\"]}}}}".to_string(),
                metadata: Some(VerificationMetadata {
                    chain_id: Some("1".into()),
                    contract_address: Some("0xcafecafecafecafecafecafecafecafecafecafe".into())
                }),
                post_actions: vec![],
            };
            let input: CompilerInput = serde_json::from_str(&request.input).unwrap();

            let verification_request: VerificationRequest = request
                .clone()
                .try_into()
                .expect("Try_into verification request failed");

            let mut expected = VerificationRequest {
                creation_bytecode: Some(DisplayBytes::from_str("0x1234").unwrap().0),
                deployed_bytecode: DisplayBytes::from_str("").unwrap().0,
                compiler_version: Version::from_str("v0.8.17+commit.8df45f5f").unwrap(),
                content: StandardJsonContent { input },
                chain_id: Some("1".into()),
            };

            // We cannot compare requests directly, as CompilerInput does not implement PartialEq
            assert_eq!(
                expected.creation_bytecode, verification_request.creation_bytecode,
                "creation bytecode"
            );
            assert_eq!(
                expected.deployed_bytecode, verification_request.deployed_bytecode,
                "deployed bytecode"
            );
            assert_eq!(
                expected.compiler_version, verification_request.compiler_version,
                "compiler version"
            );
            assert_eq!(
                serde_json::to_string(&expected.content.input).unwrap(),
                serde_json::to_string(&verification_request.content.input).unwrap(),
                "compiler input"
            );

            /********** Deployed Bytecode **********/

            request.bytecode_type = BytecodeType::DeployedBytecode.into();
            expected.deployed_bytecode = expected.creation_bytecode.take().unwrap();

            let verification_request: VerificationRequest = request
                .try_into()
                .expect("Deployed bytecode: try_into verification request failed");
            assert_eq!(
                expected.creation_bytecode, verification_request.creation_bytecode,
                "Invalid creation bytecode when deployed bytecode provided"
            );
            assert_eq!(
                expected.deployed_bytecode, verification_request.deployed_bytecode,
                "Invalid deployed bytecode when deployed bytecode provided"
            );
        }

        #[test]
        fn empty_metadata() {
            let request = VerifySolidityStandardJsonRequest {
                bytecode: "".to_string(),
                bytecode_type: BytecodeType::CreationInput.into(),
                compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
                input: "{\"language\": \"Solidity\", \"sources\": {\"./src/contracts/Foo.sol\": {\"content\": \"pragma solidity ^0.8.2;\\n\\ncontract Foo {\\n    function bar() external pure returns (uint256) {\\n        return 42;\\n    }\\n}\\n\"}}, \"settings\": {\"metadata\": {\"useLiteralContent\": true}, \"optimizer\": {\"enabled\": true, \"runs\": 200}, \"outputSelection\": {\"*\": {\"*\": [\"abi\", \"evm.bytecode\", \"evm.deployedBytecode\", \"evm.methodIdentifiers\"], \"\": [\"id\", \"ast\"]}}}}".to_string(),
                metadata: None,
                post_actions: vec![],
            };

            let verification_request: VerificationRequest = request
                .try_into()
                .expect("Try_into verification request failed");

            assert_eq!(
                None, verification_request.chain_id,
                "Absent verification metadata should result in absent chain id"
            )
        }
    }
}
