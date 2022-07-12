use super::types::VerificationRequest;
use crate::{
    compiler::{Compilers, Version},
    http_server::handlers::verification::{
        solidity::{
            contract_verifier::{compile_and_verify_handler, Input},
            types::StandardJson,
        },
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
    params: Json<VerificationRequest<StandardJson>>,
) -> Result<Json<VerificationResponse>, Error> {
    let params = params.into_inner();

    let compiler_input = params.content.into();
    let compiler_version =
        Version::from_str(&params.compiler_version).map_err(error::ErrorBadRequest)?;
    let input = Input {
        compiler_version,
        compiler_input,
        creation_tx_input: &params.creation_bytecode,
        deployed_bytecode: &params.deployed_bytecode,
    };
    compile_and_verify_handler(&compilers, input, false)
        .await
        .map(Json)
}
