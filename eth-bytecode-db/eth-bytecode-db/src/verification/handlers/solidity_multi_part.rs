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

#[derive(Debug, Clone, PartialEq, Eq)]
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
                    "unknown verified file extension: expected \".sol\" or \".vy\"; file_name={}",
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
