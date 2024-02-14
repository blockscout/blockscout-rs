use crate::{handlers::process_verification_request, ToHex, VerificationResponse};
use eth_bytecode_db_proto::{
    blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2, http_client::solidity_verifier_client,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VerificationRequest {
    pub compiler: String,
    pub evm_version: Option<String>,
    pub optimization_runs: Option<i32>,
    pub source_files: BTreeMap<String, String>,
    pub libraries: BTreeMap<String, String>,
}

pub async fn verify(
    eth_bytecode_db_client: &eth_bytecode_db_proto::http_client::Client,
    contracts: Vec<(&blockscout_client::Client, ethers_core::types::Address)>,
    request: VerificationRequest,
) -> VerificationResponse {
    let request_builder = |bytecode: ethers_core::types::Bytes,
                           bytecode_type: eth_bytecode_db_v2::BytecodeType,
                           metadata| {
        eth_bytecode_db_v2::VerifySolidityMultiPartRequest {
            bytecode: bytecode.to_hex(),
            bytecode_type: bytecode_type.into(),
            compiler_version: request.compiler.clone(),
            evm_version: request.evm_version.clone(),
            optimization_runs: request.optimization_runs,
            source_files: request.source_files.clone(),
            libraries: request.libraries.clone(),
            metadata: Some(metadata),
        }
    };

    process_verification_request(
        eth_bytecode_db_client,
        contracts,
        request_builder,
        solidity_verifier_client::verify_multi_part,
    )
    .await
}
