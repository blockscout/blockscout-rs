use super::{
    super::{
        client::Client,
        errors::Error,
        smart_contract_verifier::VerifyVyperMultiPartRequest,
        types::{BytecodeType, Source, SourceType, VerificationRequest},
    },
    process_verify_response,
};
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
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
    )
    .await
}
