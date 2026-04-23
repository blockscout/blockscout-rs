use crate::{blockscout, handlers::process_verification_request, Contract, VerificationResponse};
use blockscout_display_bytes::ToHex;
use eth_bytecode_db_proto::{
    blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2, http_client::vyper_verifier_client,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VerificationRequest {
    pub compiler: String,
    pub evm_version: Option<String>,
    pub source_files: BTreeMap<String, String>,
    pub interfaces: BTreeMap<String, String>,
}

pub async fn verify(
    eth_bytecode_db_client: &eth_bytecode_db_proto::http_client::Client,
    contracts: Vec<(&blockscout::Client, Contract)>,
    request: VerificationRequest,
) -> VerificationResponse {
    let request_builder = |bytecode: ethers_core::types::Bytes,
                           bytecode_type: eth_bytecode_db_v2::BytecodeType,
                           metadata| {
        eth_bytecode_db_v2::VerifyVyperMultiPartRequest {
            bytecode: bytecode.to_hex(),
            bytecode_type: bytecode_type.into(),
            compiler_version: request.compiler.clone(),
            evm_version: request.evm_version.clone(),
            source_files: request.source_files.clone(),
            interfaces: request.interfaces.clone(),
            metadata: Some(metadata),
        }
    };

    process_verification_request(
        eth_bytecode_db_client,
        contracts,
        request_builder,
        vyper_verifier_client::verify_multi_part,
    )
    .await
}
