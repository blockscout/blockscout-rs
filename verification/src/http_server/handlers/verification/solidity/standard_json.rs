use super::types::VerificationRequest;
use crate::{
    compiler::{Compilers, Version},
    http_server::{
        handlers::verification::{
            solidity::{
                contract_verifier::{compile_and_verify_handler, Input},
                types::StandardJson,
            },
            VerificationResponse,
        },
        metrics,
    },
};
use actix_web::{
    error,
    web::{self, Json},
    Error,
};
use std::str::FromStr;

#[tracing::instrument(skip(compilers), level = "debug", name = "verify-json")]
pub async fn verify(
    compilers: web::Data<Compilers>,
    params: Json<VerificationRequest<StandardJson>>,
) -> Result<Json<VerificationResponse>, Error> {
    let params = params.into_inner();

    let compiler_input = match params.content.try_into() {
        Ok(compiler_input) => compiler_input,
        Err(err) => return Ok(Json(VerificationResponse::err(err))),
    };
    let compiler_version =
        Version::from_str(&params.compiler_version).map_err(error::ErrorBadRequest)?;
    let input = Input {
        compiler_version,
        compiler_input,
        creation_tx_input: &params.creation_bytecode,
        deployed_bytecode: &params.deployed_bytecode,
    };
    let response = compile_and_verify_handler(&compilers, input, false).await?;
    metrics::count_verify_contract(&response.status, "json");
    Ok(Json(response))
}
