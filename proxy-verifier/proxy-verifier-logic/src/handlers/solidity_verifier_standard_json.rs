use crate::{handlers::process_verification_request, ToHex, VerificationResponse};
use eth_bytecode_db_proto::{
    blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2, http_client::solidity_verifier_client,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VerificationRequest {
    pub compiler: String,
    pub input: String,
}

pub async fn verify(
    eth_bytecode_db_client: &eth_bytecode_db_proto::http_client::Client,
    contracts: Vec<(&blockscout_client::Client, ethers_core::types::Address)>,
    request: VerificationRequest,
) -> VerificationResponse {
    let request_builder = |bytecode: ethers_core::types::Bytes,
                           bytecode_type: eth_bytecode_db_v2::BytecodeType,
                           metadata| {
        eth_bytecode_db_v2::VerifySolidityStandardJsonRequest {
            bytecode: bytecode.to_hex(),
            bytecode_type: bytecode_type.into(),
            compiler_version: request.compiler.clone(),
            input: request.input.clone(),
            metadata: Some(metadata),
        }
    };

    process_verification_request(
        eth_bytecode_db_client,
        contracts,
        request_builder,
        solidity_verifier_client::verify_standard_json,
    )
    .await
}
