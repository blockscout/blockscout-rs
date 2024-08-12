use super::StandardJsonParseError;
use crate::proto::{BytecodeType, VerifySolidityStandardJsonRequest};
use anyhow::anyhow;
use blockscout_display_bytes::Bytes as DisplayBytes;
use foundry_compilers::CompilerInput;
use serde::{Deserialize, Serialize};
use smart_contract_verifier::{
    solidity::standard_json::{StandardJsonContent, VerificationRequest},
    DetailedVersion,
};
use std::{ops::Deref, str::FromStr};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct VerifySolidityStandardJsonRequestWrapper(VerifySolidityStandardJsonRequest);

impl From<VerifySolidityStandardJsonRequest> for VerifySolidityStandardJsonRequestWrapper {
    fn from(inner: VerifySolidityStandardJsonRequest) -> Self {
        Self(inner)
    }
}

impl Deref for VerifySolidityStandardJsonRequestWrapper {
    type Target = VerifySolidityStandardJsonRequest;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl VerifySolidityStandardJsonRequestWrapper {
    pub fn new(inner: VerifySolidityStandardJsonRequest) -> Self {
        Self(inner)
    }

    pub fn into_inner(self) -> VerifySolidityStandardJsonRequest {
        self.0
    }
}

impl TryFrom<VerifySolidityStandardJsonRequestWrapper> for VerificationRequest {
    type Error = StandardJsonParseError;

    fn try_from(request: VerifySolidityStandardJsonRequestWrapper) -> Result<Self, Self::Error> {
        let request = request.into_inner();

        let bytecode = DisplayBytes::from_str(&request.bytecode)
            .map_err(|err| anyhow!("Invalid deployed bytecode: {:?}", err))?
            .0;
        let (creation_bytecode, deployed_bytecode) = match request.bytecode_type() {
            BytecodeType::Unspecified => Err(StandardJsonParseError::BadRequest(anyhow!(
                "Bytecode type is unspecified"
            )))?,
            BytecodeType::CreationInput => (Some(bytecode), bytes::Bytes::new()),
            BytecodeType::DeployedBytecode => (None, bytecode),
        };
        let compiler_version = DetailedVersion::from_str(&request.compiler_version)
            .map_err(|err| anyhow!("Invalid compiler version: {}", err))?;

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
    use crate::proto::VerificationMetadata;
    use pretty_assertions::assert_eq;

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

        let verification_request: VerificationRequest =
            <VerifySolidityStandardJsonRequestWrapper>::from(request.clone())
                .try_into()
                .expect("Try_into verification request failed");

        let mut expected = VerificationRequest {
            creation_bytecode: Some(DisplayBytes::from_str("0x1234").unwrap().0),
            deployed_bytecode: DisplayBytes::from_str("").unwrap().0,
            compiler_version: DetailedVersion::from_str("v0.8.17+commit.8df45f5f").unwrap(),
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

        let verification_request: VerificationRequest =
            <VerifySolidityStandardJsonRequestWrapper>::from(request)
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

        let verification_request: VerificationRequest =
            <VerifySolidityStandardJsonRequestWrapper>::from(request)
                .try_into()
                .expect("Try_into verification request failed");

        assert_eq!(
            None, verification_request.chain_id,
            "Absent verification metadata should result in absent chain id"
        )
    }
}
