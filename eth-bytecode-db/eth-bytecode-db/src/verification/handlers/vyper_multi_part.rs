use super::{
    super::{
        client::Client,
        errors::Error,
        smart_contract_verifier::{BytecodeType, VerifyVyperMultiPartRequest},
        types::{Source, VerificationRequest, VerificationType},
    },
    process_verify_response, EthBytecodeDbAction, VerifierAllianceDbAction,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiPartFiles {
    pub evm_version: Option<String>,
    pub source_files: BTreeMap<String, String>,
    pub interfaces: BTreeMap<String, String>,
}

impl From<VerificationRequest<MultiPartFiles>> for VerifyVyperMultiPartRequest {
    fn from(request: VerificationRequest<MultiPartFiles>) -> Self {
        Self {
            bytecode: request.bytecode,
            bytecode_type: BytecodeType::from(request.bytecode_type).into(),
            compiler_version: request.compiler_version,
            source_files: request.content.source_files,
            interfaces: request.content.interfaces,
            evm_version: request.content.evm_version,
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

    let request: VerifyVyperMultiPartRequest = request.into();
    let response = client
        .vyper_client
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
                evm_version: Some("istanbul".to_string()),
                source_files: BTreeMap::from([
                    ("source_file1".into(), "content1".into()),
                    ("source_file2".into(), "content2".into()),
                ]),
                interfaces: BTreeMap::from([
                    ("interface1".into(), "interface_content1".into()),
                    ("interface2".into(), "interface_content2".into()),
                ]),
            },
            metadata: Some(types::VerificationMetadata {
                chain_id: Some(1),
                contract_address: Some(bytes::Bytes::from_static(&[1u8; 20])),
                ..Default::default()
            }),
            is_authorized: false,
        };
        let expected = VerifyVyperMultiPartRequest {
            bytecode: "0x1234".to_string(),
            bytecode_type: BytecodeType::CreationInput.into(),
            compiler_version: "compiler_version".to_string(),
            source_files: BTreeMap::from([
                ("source_file1".into(), "content1".into()),
                ("source_file2".into(), "content2".into()),
            ]),
            interfaces: BTreeMap::from([
                ("interface1".into(), "interface_content1".into()),
                ("interface2".into(), "interface_content2".into()),
            ]),
            evm_version: Some("istanbul".to_string()),
            metadata: Some(smart_contract_verifier::VerificationMetadata {
                chain_id: Some("1".to_string()),
                contract_address: Some("0x0101010101010101010101010101010101010101".to_string()),
            }),
        };
        assert_eq!(
            expected,
            VerifyVyperMultiPartRequest::from(request),
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
                evm_version: Some("istanbul".to_string()),
                source_files: BTreeMap::from([
                    ("source_file1".into(), "content1".into()),
                    ("source_file2".into(), "content2".into()),
                ]),
                interfaces: BTreeMap::from([
                    ("interface1".into(), "interface_content1".into()),
                    ("interface2".into(), "interface_content2".into()),
                ]),
            },
            metadata: Some(types::VerificationMetadata {
                chain_id: Some(1),
                contract_address: Some(bytes::Bytes::from_static(&[1u8; 20])),
                ..Default::default()
            }),
            is_authorized: false,
        };
        let expected = VerifyVyperMultiPartRequest {
            bytecode: "0x1234".to_string(),
            bytecode_type: BytecodeType::DeployedBytecode.into(),
            compiler_version: "compiler_version".to_string(),
            source_files: BTreeMap::from([
                ("source_file1".into(), "content1".into()),
                ("source_file2".into(), "content2".into()),
            ]),
            interfaces: BTreeMap::from([
                ("interface1".into(), "interface_content1".into()),
                ("interface2".into(), "interface_content2".into()),
            ]),
            evm_version: Some("istanbul".to_string()),
            metadata: Some(smart_contract_verifier::VerificationMetadata {
                chain_id: Some("1".to_string()),
                contract_address: Some("0x0101010101010101010101010101010101010101".to_string()),
            }),
        };
        assert_eq!(
            expected,
            VerifyVyperMultiPartRequest::from(request),
            "Invalid conversion"
        );
    }
}
