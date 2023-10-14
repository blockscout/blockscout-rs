use super::{
    super::{
        client::Client,
        errors::Error,
        smart_contract_verifier::{BytecodeType, VerifySolidityMultiPartRequest},
        types::{Source, VerificationRequest, VerificationType},
    },
    process_verify_response, EthBytecodeDbAction, VerifierAllianceDbAction,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiPartFiles {
    pub source_files: BTreeMap<String, String>,
    pub evm_version: Option<String>,
    pub optimization_runs: Option<i32>,
    pub libraries: BTreeMap<String, String>,
}

impl From<VerificationRequest<MultiPartFiles>> for VerifySolidityMultiPartRequest {
    fn from(request: VerificationRequest<MultiPartFiles>) -> Self {
        Self {
            bytecode: request.bytecode,
            bytecode_type: BytecodeType::from(request.bytecode_type).into(),
            compiler_version: request.compiler_version,
            source_files: request.content.source_files,
            evm_version: request.content.evm_version,
            optimization_runs: request.content.optimization_runs,
            libraries: request.content.libraries,
            metadata: request.metadata.map(|metadata| metadata.into()),
        }
    }
}

pub async fn verify(
    mut client: Client,
    request: VerificationRequest<MultiPartFiles>,
) -> Result<Source, Error> {
    let is_authorized = request.is_authorized;
    let bytecode_type = request.bytecode_type;
    let raw_request_bytecode = hex::decode(request.bytecode.clone().trim_start_matches("0x"))
        .map_err(|err| Error::InvalidArgument(format!("invalid bytecode: {err}")))?;
    let verification_settings = serde_json::json!(&request);
    let verification_metadata = request.metadata.clone();

    let request: VerifySolidityMultiPartRequest = request.into();
    let response = client
        .solidity_client
        .verify_multi_part(request)
        .await
        .map_err(Error::from)?
        .into_inner();

    let verifier_alliance_db_action = VerifierAllianceDbAction::from_db_client_and_metadata(
        client.alliance_db_client.as_deref(),
        verification_metadata.clone(),
        is_authorized,
    );
    process_verify_response(
        response,
        EthBytecodeDbAction::SaveData {
            db_client: &client.db_client,
            bytecode_type,
            raw_request_bytecode,
            verification_settings,
            verification_type: VerificationType::MultiPartFiles,
            verification_metadata,
        },
        verifier_alliance_db_action,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{
        super::super::{smart_contract_verifier, types},
        *,
    };
    use pretty_assertions::assert_eq;

    #[test]
    fn from_verification_request_creation_input() {
        let request = VerificationRequest {
            bytecode: "0x1234".to_string(),
            bytecode_type: types::BytecodeType::CreationInput,
            compiler_version: "compiler_version".to_string(),
            content: MultiPartFiles {
                source_files: BTreeMap::from([
                    ("source_file1".into(), "content1".into()),
                    ("source_file2".into(), "content2".into()),
                ]),
                evm_version: Some("london".to_string()),
                optimization_runs: Some(200),
                libraries: BTreeMap::from([("lib1".into(), "0xcafe".into())]),
            },
            metadata: Some(types::VerificationMetadata {
                chain_id: Some(1),
                contract_address: Some(bytes::Bytes::from_static(&[1u8; 20])),
                ..Default::default()
            }),
            is_authorized: false,
        };
        let expected = VerifySolidityMultiPartRequest {
            bytecode: "0x1234".to_string(),
            bytecode_type: BytecodeType::CreationInput.into(),
            compiler_version: "compiler_version".to_string(),
            source_files: BTreeMap::from([
                ("source_file1".into(), "content1".into()),
                ("source_file2".into(), "content2".into()),
            ]),
            evm_version: Some("london".to_string()),
            optimization_runs: Some(200),
            libraries: BTreeMap::from([("lib1".into(), "0xcafe".into())]),
            metadata: Some(smart_contract_verifier::VerificationMetadata {
                chain_id: Some("1".to_string()),
                contract_address: Some("0x0101010101010101010101010101010101010101".to_string()),
            }),
        };
        assert_eq!(
            expected,
            VerifySolidityMultiPartRequest::from(request),
            "Invalid conversion"
        );
    }

    #[test]
    fn from_verification_request_deployed_bytecode() {
        let request = VerificationRequest {
            bytecode: "0x1234".to_string(),
            bytecode_type: types::BytecodeType::DeployedBytecode,
            compiler_version: "compiler_version".to_string(),
            content: MultiPartFiles {
                source_files: BTreeMap::from([
                    ("source_file1".into(), "content1".into()),
                    ("source_file2".into(), "content2".into()),
                ]),
                evm_version: Some("london".to_string()),
                optimization_runs: Some(200),
                libraries: BTreeMap::from([("lib1".into(), "0xcafe".into())]),
            },
            metadata: Some(types::VerificationMetadata {
                chain_id: Some(1),
                contract_address: Some(bytes::Bytes::from_static(&[1u8; 20])),
                ..Default::default()
            }),
            is_authorized: false,
        };
        let expected = VerifySolidityMultiPartRequest {
            bytecode: "0x1234".to_string(),
            bytecode_type: BytecodeType::DeployedBytecode.into(),
            compiler_version: "compiler_version".to_string(),
            source_files: BTreeMap::from([
                ("source_file1".into(), "content1".into()),
                ("source_file2".into(), "content2".into()),
            ]),
            evm_version: Some("london".to_string()),
            optimization_runs: Some(200),
            libraries: BTreeMap::from([("lib1".into(), "0xcafe".into())]),
            metadata: Some(smart_contract_verifier::VerificationMetadata {
                chain_id: Some("1".to_string()),
                contract_address: Some("0x0101010101010101010101010101010101010101".to_string()),
            }),
        };
        assert_eq!(
            expected,
            VerifySolidityMultiPartRequest::from(request),
            "Invalid conversion"
        );
    }
}
