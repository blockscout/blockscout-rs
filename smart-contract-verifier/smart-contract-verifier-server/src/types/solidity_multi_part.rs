use blockscout_display_bytes::Bytes as DisplayBytes;
use ethers_solc::EvmVersion;
use serde::{Deserialize, Serialize};
use smart_contract_verifier::{
    solidity::multi_part::{MultiFileContent, VerificationRequest},
    Version,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::VerifySolidityMultiPartRequest;
use std::{collections::BTreeMap, ops::Deref, path::PathBuf, str::FromStr};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct VerifySolidityMultiPartRequestWrapper(VerifySolidityMultiPartRequest);

impl From<VerifySolidityMultiPartRequest> for VerifySolidityMultiPartRequestWrapper {
    fn from(inner: VerifySolidityMultiPartRequest) -> Self {
        Self(inner)
    }
}

impl Deref for VerifySolidityMultiPartRequestWrapper {
    type Target = VerifySolidityMultiPartRequest;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl VerifySolidityMultiPartRequestWrapper {
    pub fn new(inner: VerifySolidityMultiPartRequest) -> Self {
        Self(inner)
    }

    pub fn into_inner(self) -> VerifySolidityMultiPartRequest {
        self.0
    }
}

impl TryFrom<VerifySolidityMultiPartRequestWrapper> for VerificationRequest {
    type Error = tonic::Status;

    fn try_from(request: VerifySolidityMultiPartRequestWrapper) -> Result<Self, Self::Error> {
        let request = request.into_inner();

        let deployed_bytecode = DisplayBytes::from_str(&request.deployed_bytecode)
            .map_err(|err| {
                tonic::Status::invalid_argument(format!("Invalid deployed bytecode: {:?}", err))
            })?
            .0;
        let creation_bytecode = match &request.creation_tx_input {
            None => None,
            Some(creation_bytecode) => Some(
                DisplayBytes::from_str(creation_bytecode)
                    .map_err(|err| {
                        tonic::Status::invalid_argument(format!(
                            "Invalid creation bytecode: {:?}",
                            err
                        ))
                    })?
                    .0,
            ),
        };
        let compiler_version = Version::from_str(&request.compiler_version).map_err(|err| {
            tonic::Status::invalid_argument(format!("Invalid compiler version: {}", err))
        })?;

        let sources: BTreeMap<PathBuf, String> = request
            .sources
            .into_iter()
            .map(|(name, content)| (PathBuf::from_str(&name).unwrap(), content))
            .collect();

        let evm_version = if request.evm_version != "default" {
            Some(
                EvmVersion::from_str(&request.evm_version)
                    .map_err(tonic::Status::invalid_argument)?,
            )
        } else {
            None
        };

        Ok(Self {
            deployed_bytecode,
            creation_bytecode,
            compiler_version,
            content: MultiFileContent {
                sources,
                evm_version,
                optimization_runs: request.optimization_runs.map(|i| i as usize),
                contract_libraries: Some(request.libraries.into_iter().collect()),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn try_into_verification_request() {
        let request = VerifySolidityMultiPartRequest {
            creation_tx_input: Some("0x1234".to_string()),
            deployed_bytecode: "0x5678".to_string(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            sources: BTreeMap::from([("source_path".into(), "source_content".into())]),
            evm_version: "london".to_string(),
            optimization_runs: Some(200),
            libraries: BTreeMap::from([("Lib".into(), "0xcafe".into())]),
        };

        let verification_request: VerificationRequest =
            <VerifySolidityMultiPartRequestWrapper>::from(request)
                .try_into()
                .expect("Try_into verification request failed");

        let expected = VerificationRequest {
            creation_bytecode: Some(DisplayBytes::from_str("0x1234").unwrap().0),
            deployed_bytecode: DisplayBytes::from_str("0x5678").unwrap().0,
            compiler_version: Version::from_str("v0.8.17+commit.8df45f5f").unwrap(),
            content: MultiFileContent {
                sources: BTreeMap::from([("source_path".into(), "source_content".into())]),
                evm_version: Some(EvmVersion::London),
                optimization_runs: Some(200),
                contract_libraries: Some(BTreeMap::from([("Lib".into(), "0xcafe".into())])),
            },
        };

        assert_eq!(expected, verification_request);
    }

    #[test]
    // 'default' should result in None in MultiFileContent
    fn default_evm_version() {
        let request = VerifySolidityMultiPartRequest {
            creation_tx_input: None,
            deployed_bytecode: "".to_string(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            sources: Default::default(),
            evm_version: "default".to_string(),
            optimization_runs: None,
            libraries: Default::default(),
        };

        let verification_request: VerificationRequest =
            <VerifySolidityMultiPartRequestWrapper>::from(request)
                .try_into()
                .expect("Try_into verification request failed");

        assert_eq!(
            None, verification_request.content.evm_version,
            "'default' should result in `None`"
        )
    }
}
