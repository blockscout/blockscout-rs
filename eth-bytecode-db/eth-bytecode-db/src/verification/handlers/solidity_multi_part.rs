use super::{
    super::{
        client::Client,
        errors::Error,
        smart_contract_verifier::VerifySolidityMultiPartRequest,
        types::{BytecodeType, Source, SourceType, VerificationRequest},
    },
    process_verify_response,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultiPartFiles {
    pub source_files: BTreeMap<String, String>,
    pub evm_version: String,
    pub optimization_runs: Option<i32>,
    pub libraries: BTreeMap<String, String>,
}

impl From<VerificationRequest<MultiPartFiles>> for VerifySolidityMultiPartRequest {
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
            evm_version: request.content.evm_version,
            optimization_runs: request.content.optimization_runs,
            contract_libraries: request.content.libraries,
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

    let request: VerifySolidityMultiPartRequest = request.into();
    let response = client
        .solidity_client
        .verify_multi_part(request)
        .await
        .map_err(Error::from)?
        .into_inner();

    let source_type_fn = |file_name: &str| {
        if file_name.ends_with(".sol") {
            Ok(SourceType::Solidity)
        } else if file_name.ends_with(".yul") {
            Ok(SourceType::Yul)
        } else {
            Err(Error::Internal(
                anyhow::anyhow!(
                    "unknown verified file extension: expected \".sol\" or \".yul\"; file_name={}",
                    file_name
                )
                .context("verifier service connection"),
            ))
        }
    };

    process_verify_response(
        &client.db_client,
        response,
        bytecode_type,
        raw_request_bytecode,
        source_type_fn,
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
                source_files: BTreeMap::from([
                    ("source_file1".into(), "content1".into()),
                    ("source_file2".into(), "content2".into()),
                ]),
                evm_version: "london".to_string(),
                optimization_runs: Some(200),
                libraries: BTreeMap::from([("lib1".into(), "0xcafe".into())]),
            },
        };
        let expected = VerifySolidityMultiPartRequest {
            creation_bytecode: Some("0x1234".to_string()),
            deployed_bytecode: "".to_string(),
            compiler_version: "compiler_version".to_string(),
            sources: BTreeMap::from([
                ("source_file1".into(), "content1".into()),
                ("source_file2".into(), "content2".into()),
            ]),
            evm_version: "london".to_string(),
            optimization_runs: Some(200),
            contract_libraries: BTreeMap::from([("lib1".into(), "0xcafe".into())]),
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
            bytecode_type: BytecodeType::DeployedBytecode,
            compiler_version: "compiler_version".to_string(),
            content: MultiPartFiles {
                source_files: BTreeMap::from([
                    ("source_file1".into(), "content1".into()),
                    ("source_file2".into(), "content2".into()),
                ]),
                evm_version: "london".to_string(),
                optimization_runs: Some(200),
                libraries: BTreeMap::from([("lib1".into(), "0xcafe".into())]),
            },
        };
        let expected = VerifySolidityMultiPartRequest {
            creation_bytecode: None,
            deployed_bytecode: "0x1234".to_string(),
            compiler_version: "compiler_version".to_string(),
            sources: BTreeMap::from([
                ("source_file1".into(), "content1".into()),
                ("source_file2".into(), "content2".into()),
            ]),
            evm_version: "london".to_string(),
            optimization_runs: Some(200),
            contract_libraries: BTreeMap::from([("lib1".into(), "0xcafe".into())]),
        };
        assert_eq!(
            expected,
            VerifySolidityMultiPartRequest::from(request),
            "Invalid conversion"
        );
    }
}
