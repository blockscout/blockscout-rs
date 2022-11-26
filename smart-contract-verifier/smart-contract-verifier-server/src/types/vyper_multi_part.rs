use blockscout_display_bytes::Bytes as DisplayBytes;
use ethers_solc::EvmVersion;
use serde::{Deserialize, Serialize};
use smart_contract_verifier::{
    vyper::multi_part::{MultiFileContent, VerificationRequest},
    Version,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::VerifyVyperMultiPartRequest;
use std::{collections::BTreeMap, ops::Deref, path::PathBuf, str::FromStr};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct VerifyVyperMultiPartRequestWrapper(VerifyVyperMultiPartRequest);

impl From<VerifyVyperMultiPartRequest> for VerifyVyperMultiPartRequestWrapper {
    fn from(inner: VerifyVyperMultiPartRequest) -> Self {
        Self(inner)
    }
}

impl Deref for VerifyVyperMultiPartRequestWrapper {
    type Target = VerifyVyperMultiPartRequest;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl VerifyVyperMultiPartRequestWrapper {
    pub fn new(inner: VerifyVyperMultiPartRequest) -> Self {
        Self(inner)
    }

    pub fn into_inner(self) -> VerifyVyperMultiPartRequest {
        self.0
    }
}

impl TryFrom<VerifyVyperMultiPartRequestWrapper> for VerificationRequest {
    type Error = tonic::Status;

    fn try_from(request: VerifyVyperMultiPartRequestWrapper) -> Result<Self, Self::Error> {
        let request = request.into_inner();

        let deployed_bytecode = DisplayBytes::from_str(&request.deployed_bytecode)
            .map_err(|err| {
                tonic::Status::invalid_argument(format!("Invalid deployed bytecode: {:?}", err))
            })?
            .0;
        let creation_bytecode = match &request.creation_bytecode {
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

        let evm_version = if let Some(version) = request.evm_version {
            Some(EvmVersion::from_str(&version).map_err(tonic::Status::invalid_argument)?)
        } else {
            // default evm version for vyper
            Some(EvmVersion::Istanbul)
        };

        Ok(Self {
            deployed_bytecode,
            creation_bytecode,
            compiler_version,
            content: MultiFileContent {
                sources,
                evm_version,
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
        let request = VerifyVyperMultiPartRequest {
            creation_bytecode: Some("0x1234".to_string()),
            deployed_bytecode: "0x5678".to_string(),
            compiler_version: "0.3.7+commit.6020b8bb".to_string(),
            sources: BTreeMap::from([("source_path".into(), "source_content".into())]),
            evm_version: Some("byzantium".to_string()),
        };

        let verification_request: VerificationRequest =
            <VerifyVyperMultiPartRequestWrapper>::from(request)
                .try_into()
                .expect("Try_into verification request failed");

        let expected = VerificationRequest {
            creation_bytecode: Some(DisplayBytes::from_str("0x1234").unwrap().0),
            deployed_bytecode: DisplayBytes::from_str("0x5678").unwrap().0,
            compiler_version: Version::from_str("0.3.7+commit.6020b8bb").unwrap(),
            content: MultiFileContent {
                sources: BTreeMap::from([("source_path".into(), "source_content".into())]),
                evm_version: Some(EvmVersion::Byzantium),
            },
        };

        assert_eq!(expected, verification_request);
    }

    #[test]
    // 'None' should result in "EvmVersion::Istanbul"
    fn default_evm_version() {
        let request = VerifyVyperMultiPartRequest {
            creation_bytecode: None,
            deployed_bytecode: "".to_string(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            sources: Default::default(),
            evm_version: None,
        };

        let verification_request: VerificationRequest =
            <VerifyVyperMultiPartRequestWrapper>::from(request)
                .try_into()
                .expect("Try_into verification request failed");

        assert_eq!(
            Some(EvmVersion::Istanbul),
            verification_request.content.evm_version,
            "'default' should result in `None`"
        )
    }
}
