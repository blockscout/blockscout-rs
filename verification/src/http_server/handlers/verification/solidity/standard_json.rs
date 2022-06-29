use super::types::VerificationRequest;
use crate::{
    compiler::{CompilerVersion, Compilers},
    http_server::handlers::verification::{
        solidity::{
            handlers::{compile_and_verify_handler, CompileAndVerifyInput},
            types::StandardJson,
        },
        VerificationResponse,
    },
    solidity::CompilerFetcher,
};
use actix_web::{
    error,
    web::{self, Json},
    Error,
};
use std::str::FromStr;

pub async fn verify(
    compilers: web::Data<Compilers<CompilerFetcher>>,
    params: Json<VerificationRequest<StandardJson>>,
) -> Result<Json<VerificationResponse>, Error> {
    let params = params.into_inner();

    let compiler_input = params.content.into();
    let compiler_version =
        CompilerVersion::from_str(&params.compiler_version).map_err(error::ErrorBadRequest)?;
    let input = CompileAndVerifyInput {
        compiler_version,
        compiler_input,
        creation_tx_input: &params.creation_bytecode,
        deployed_bytecode: &params.deployed_bytecode,
    };
    compile_and_verify_handler(&compilers, input)
        .await
        .map(Json)
}
