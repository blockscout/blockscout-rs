use super::types::{MultiPartFiles, VerificationRequest};
use crate::{
    compiler::{Compilers, Version},
    http_server::handlers::verification::{
        solidity::contract_verifier::{compile_and_verify_handler, Input},
        VerificationResponse,
    },
};
use actix_web::{
    error,
    web::{self, Json},
    Error,
};
use std::str::FromStr;

pub async fn verify(
    compilers: web::Data<Compilers>,
    params: Json<VerificationRequest<MultiPartFiles>>,
) -> Result<Json<VerificationResponse>, Error> {
    let params = params.into_inner();

    let compiler_input = params.content.try_into().map_err(error::ErrorBadRequest)?;
    let compiler_version =
        Version::from_str(&params.compiler_version).map_err(error::ErrorBadRequest)?;
    let input = Input {
        compiler_version,
        compiler_input,
        creation_tx_input: &params.creation_bytecode,
        deployed_bytecode: &params.deployed_bytecode,
    };
    compile_and_verify_handler(&compilers, input, true)
        .await
        .map(Json)
}
