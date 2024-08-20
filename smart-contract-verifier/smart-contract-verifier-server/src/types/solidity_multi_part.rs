use crate::proto::{BytecodeType, VerifySolidityMultiPartRequest};
use blockscout_display_bytes::Bytes as DisplayBytes;
use foundry_compilers::EvmVersion;
use serde::{Deserialize, Serialize};
use smart_contract_verifier::{
    solidity::multi_part::{MultiFileContent, VerificationRequest},
    DetailedVersion,
};
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

        let bytecode = DisplayBytes::from_str(&request.bytecode)
            .map_err(|err| tonic::Status::invalid_argument(format!("Invalid bytecode: {err:?}")))?
            .0;
        let (creation_bytecode, deployed_bytecode) = match request.bytecode_type() {
            BytecodeType::Unspecified => Err(tonic::Status::invalid_argument(
                "bytecode type is unspecified",
            ))?,
            BytecodeType::CreationInput => (Some(bytecode), bytes::Bytes::new()),
            BytecodeType::DeployedBytecode => (None, bytecode),
        };

        let compiler_version =
            DetailedVersion::from_str(&request.compiler_version).map_err(|err| {
                tonic::Status::invalid_argument(format!("Invalid compiler version: {err}"))
            })?;

        let sources: BTreeMap<PathBuf, String> = request
            .source_files
            .into_iter()
            .map(|(name, content)| {
                (
                    PathBuf::from_str(&name).unwrap(), /* TODO: why unwrap? */
                    content,
                )
            })
            .collect();

        let evm_version = match request.evm_version {
            Some(version) if version != "default" => {
                Some(EvmVersion::from_str(&version).map_err(tonic::Status::invalid_argument)?)
            }
            _ => None,
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

        let mut request = VerifySolidityMultiPartRequest {
            bytecode: "0x1234".to_string(),
            bytecode_type: BytecodeType::CreationInput.into(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            source_files: BTreeMap::from([("source_path".into(), "source_content".into())]),
            evm_version: Some("london".to_string()),
            optimization_runs: Some(200),
            libraries: BTreeMap::from([("Lib".into(), "0xcafe".into())]),
            metadata: Some(VerificationMetadata {
                chain_id: Some("1".into()),
                contract_address: Some("0xcafecafecafecafecafecafecafecafecafecafe".into()),
            }),
            post_actions: vec![],
        };

        let mut expected = VerificationRequest {
            creation_bytecode: Some(DisplayBytes::from_str("0x1234").unwrap().0),
            deployed_bytecode: DisplayBytes::from_str("").unwrap().0,
            compiler_version: DetailedVersion::from_str("v0.8.17+commit.8df45f5f").unwrap(),
            content: MultiFileContent {
                sources: BTreeMap::from([("source_path".into(), "source_content".into())]),
                evm_version: Some(EvmVersion::London),
                optimization_runs: Some(200),
                contract_libraries: Some(BTreeMap::from([("Lib".into(), "0xcafe".into())])),
            },
            chain_id: Some("1".into()),
        };

        let verification_request: VerificationRequest =
            <VerifySolidityMultiPartRequestWrapper>::from(request.clone())
                .try_into()
                .expect("Creation input: try_into verification request failed");
        assert_eq!(expected, verification_request, "Creation input");

        /********** Deployed Bytecode **********/

        request.bytecode_type = BytecodeType::DeployedBytecode.into();
        expected.deployed_bytecode = expected.creation_bytecode.take().unwrap();

        let verification_request: VerificationRequest =
            <VerifySolidityMultiPartRequestWrapper>::from(request)
                .try_into()
                .expect("Deployed bytecode: try_into verification request failed");
        assert_eq!(expected, verification_request, "Deployed bytecode");
    }

    #[test]
    // 'default' should result in None in MultiFileContent
    fn default_evm_version() {
        let request = VerifySolidityMultiPartRequest {
            bytecode: "".to_string(),
            bytecode_type: BytecodeType::CreationInput.into(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            source_files: Default::default(),
            evm_version: Some("default".to_string()),
            optimization_runs: None,
            libraries: Default::default(),
            metadata: None,
            post_actions: vec![],
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

    #[test]
    // 'null' should result in None in MultiFileContent
    fn null_evm_version() {
        let request = VerifySolidityMultiPartRequest {
            bytecode: "".to_string(),
            bytecode_type: BytecodeType::CreationInput.into(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            source_files: Default::default(),
            evm_version: None,
            optimization_runs: None,
            libraries: Default::default(),
            metadata: None,
            post_actions: vec![],
        };

        let verification_request: VerificationRequest =
            <VerifySolidityMultiPartRequestWrapper>::from(request)
                .try_into()
                .expect("Try_into verification request failed");

        assert_eq!(
            None, verification_request.content.evm_version,
            "Absent evm_version should result in `None`"
        )
    }

    #[test]
    fn empty_metadata() {
        let request = VerifySolidityMultiPartRequest {
            bytecode: "".to_string(),
            bytecode_type: BytecodeType::CreationInput.into(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            source_files: Default::default(),
            evm_version: None,
            optimization_runs: None,
            libraries: Default::default(),
            metadata: None,
            post_actions: vec![],
        };

        let verification_request: VerificationRequest =
            <VerifySolidityMultiPartRequestWrapper>::from(request)
                .try_into()
                .expect("Try_into verification request failed");

        assert_eq!(
            None, verification_request.chain_id,
            "Absent verification metadata should result in chain_id=None"
        )
    }
}
