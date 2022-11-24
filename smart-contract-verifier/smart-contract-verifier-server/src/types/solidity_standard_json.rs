use anyhow::anyhow;
use blockscout_display_bytes::Bytes as DisplayBytes;
use ethers_solc::CompilerInput;
use serde::{Deserialize, Serialize};
use smart_contract_verifier::{
    solidity::standard_json::{StandardJsonContent, VerificationRequest},
    Version,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::VerifySolidityStandardJsonRequest;
use std::{ops::Deref, str::FromStr};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("content is not valid standard json: {0}")]
    InvalidContent(#[from] serde_json::Error),
    #[error("{0}")]
    BadRequest(#[from] anyhow::Error),
}

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
    type Error = ParseError;

    fn try_from(request: VerifySolidityStandardJsonRequestWrapper) -> Result<Self, Self::Error> {
        let request = request.into_inner();

        let deployed_bytecode = DisplayBytes::from_str(&request.deployed_bytecode)
            .map_err(|err| anyhow!("Invalid deployed bytecode: {:?}", err))?
            .0;
        let creation_bytecode = match request.creation_tx_input {
            None => None,
            Some(creation_bytecode) => Some(
                DisplayBytes::from_str(&creation_bytecode)
                    .map_err(|err| anyhow!("Invalid creation bytecode: {:?}", err))?
                    .0,
            ),
        };
        let compiler_version = Version::from_str(&request.compiler_version)
            .map_err(|err| anyhow!("Invalid compiler version: {}", err))?;

        let input: CompilerInput = serde_json::from_str(&request.input)?;

        Ok(Self {
            deployed_bytecode,
            creation_bytecode,
            compiler_version,
            content: StandardJsonContent { input },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_into_verification_request() {
        let request = VerifySolidityStandardJsonRequest {
            creation_tx_input: Some("0x1234".to_string()),
            deployed_bytecode: "0x5678".to_string(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            input: "{\"language\": \"Solidity\", \"sources\": {\"./src/contracts/Foo.sol\": {\"content\": \"pragma solidity ^0.8.2;\\n\\ncontract Foo {\\n    function bar() external pure returns (uint256) {\\n        return 42;\\n    }\\n}\\n\"}}, \"settings\": {\"metadata\": {\"useLiteralContent\": true}, \"optimizer\": {\"enabled\": true, \"runs\": 200}, \"outputSelection\": {\"*\": {\"*\": [\"abi\", \"evm.bytecode\", \"evm.deployedBytecode\", \"evm.methodIdentifiers\"], \"\": [\"id\", \"ast\"]}}}}".to_string()
        };
        let input: CompilerInput = serde_json::from_str(&request.input).unwrap();

        let verification_request: VerificationRequest =
            <VerifySolidityStandardJsonRequestWrapper>::from(request)
                .try_into()
                .expect("Try_into verification request failed");

        let expected = VerificationRequest {
            creation_bytecode: Some(DisplayBytes::from_str("0x1234").unwrap().0),
            deployed_bytecode: DisplayBytes::from_str("0x5678").unwrap().0,
            compiler_version: Version::from_str("v0.8.17+commit.8df45f5f").unwrap(),
            content: StandardJsonContent { input },
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
    }
}
