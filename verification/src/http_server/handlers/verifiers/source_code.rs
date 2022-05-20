use actix_web::{web::Json, Error};
use serde::{Deserialize, Serialize};

use super::common::CommonFields;

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
pub struct VerificateRequest {
    common_fields: CommonFields,
    flattended_source: FlattenedSource,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct VerificateResponse {
    verificated: bool,
}

pub async fn verificate(
    params: Json<VerificateRequest>,
) -> Result<Json<VerificateResponse>, Error> {
    println!("{:?}", params);
    Ok(Json(VerificateResponse { verificated: true }))
}
