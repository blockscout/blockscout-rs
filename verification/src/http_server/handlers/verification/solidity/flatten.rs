use actix_web::{web::Json, Error};
use serde::{Deserialize, Serialize};

use super::base_input::VerificationBase;

use log::debug;

#[derive(Debug, Deserialize)]
pub struct ContractLibrary {
    lib_name: String,
    lib_address: String,
}

#[derive(Debug, Deserialize)]
pub struct FlattenedSource {
    source_code: String,
    evm_version: String,
    optimization: bool,
    optimization_runs: Option<u32>,
    contract_libraries: Option<Vec<ContractLibrary>>,
}

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    base: VerificationBase,
    flattended_source: FlattenedSource,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct VerifyResponse {
    verified: bool,
}

pub async fn verify(params: Json<VerifyRequest>) -> Result<Json<VerifyResponse>, Error> {
    debug!("verify contract with params {:?}", params);
    Ok(Json(VerifyResponse { verified: true }))
}
