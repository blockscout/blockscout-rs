use super::{
    super::{
        client::Client,
        errors::Error,
        smart_contract_verifier::VerifyVyperMultiPartRequest,
        types::{BytecodeType, Source, SourceType, VerificationRequest, VerificationType},
    },
    process_verify_response,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiPartFiles {
    pub evm_version: String,
    pub optimizations: bool,
    pub source_files: BTreeMap<String, String>,
}

impl From<VerificationRequest<MultiPartFiles>> for VerifyVyperMultiPartRequest {
    fn from(request: VerificationRequest<MultiPartFiles>) -> Self {
        let (creation_bytecode, deployed_bytecode) = match request.bytecode_type {
            BytecodeType::CreationInput => (Some(request.bytecode), "".to_string()),
            BytecodeType::DeployedBytecode => (None, request.bytecode),
        };
        Self {
            creation_bytecode,
            deployed_bytecode,
            compiler_version: request.compiler_version,
            sources: request.content.source_files,
            evm_version: Some(request.content.evm_version),
        }
    }
}

pub async fn verify(
    mut client: Client,
    request: VerificationRequest<MultiPartFiles>,
) -> Result<Source, Error> {
    let bytecode_type = request.bytecode_type;
    let raw_request_bytecode = hex::decode(request.bytecode.clone().trim_start_matches("0x"))
        .map_err(|err| Error::InvalidArgument(format!("invalid bytecode: {}", err)))?;
    let verification_settings = serde_json::json!(&request);

    let request: VerifyVyperMultiPartRequest = request.into();
    let response = client
        .vyper_client
        .verify_multi_part(request)
        .await
        .map_err(Error::from)?
        .into_inner();

    let source_type_fn = |_file_name: &str| Ok(SourceType::Vyper);

    process_verify_response(
        &client.db_client,
        response,
        bytecode_type,
        raw_request_bytecode,
        source_type_fn,
        verification_settings,
        VerificationType::MultiPartFiles,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn from_verification_request_creation_input() {
        let request = VerificationRequest {
            bytecode: "0x1234".to_string(),
            bytecode_type: BytecodeType::CreationInput,
            compiler_version: "compiler_version".to_string(),
            content: MultiPartFiles {
                evm_version: "istanbul".to_string(),
                optimizations: true,
                source_files: BTreeMap::from([
                    ("source_file1".into(), "content1".into()),
                    ("source_file2".into(), "content2".into()),
                ]),
            },
        };
        let expected = VerifyVyperMultiPartRequest {
            creation_bytecode: Some("0x1234".to_string()),
            deployed_bytecode: "".to_string(),
            compiler_version: "compiler_version".to_string(),
            sources: BTreeMap::from([
                ("source_file1".into(), "content1".into()),
                ("source_file2".into(), "content2".into()),
            ]),
            evm_version: Some("istanbul".to_string()),
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
            bytecode_type: BytecodeType::DeployedBytecode,
            compiler_version: "compiler_version".to_string(),
            content: MultiPartFiles {
                evm_version: "istanbul".to_string(),
                optimizations: true,
                source_files: BTreeMap::from([
                    ("source_file1".into(), "content1".into()),
                    ("source_file2".into(), "content2".into()),
                ]),
            },
        };
        let expected = VerifyVyperMultiPartRequest {
            creation_bytecode: None,
            deployed_bytecode: "0x1234".to_string(),
            compiler_version: "compiler_version".to_string(),
            sources: BTreeMap::from([
                ("source_file1".into(), "content1".into()),
                ("source_file2".into(), "content2".into()),
            ]),
            evm_version: Some("istanbul".to_string()),
        };
        assert_eq!(
            expected,
            VerifyVyperMultiPartRequest::from(request),
            "Invalid conversion"
        );
    }
}
