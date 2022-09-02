use crate::{
    compiler::{self, Compilers, Version},
    http_server::{handlers::verification::VerificationResponse, metrics},
    solidity::Verifier,
    vyper::VyperCompiler,
    VerificationResult,
};
use actix_web::{
    error,
    web::{self, Json},
    Error,
};
use ethers_solc::CompilerInput;
use std::str::FromStr;
use tracing::instrument;

use super::types::VyperVerificationRequest;

#[instrument(skip(compilers, params), level = "debug")]
pub async fn verify(
    compilers: web::Data<Compilers<VyperCompiler>>,
    params: Json<VyperVerificationRequest>,
) -> Result<Json<VerificationResponse>, Error> {
    let request = params.into_inner();
    let compiler_input =
        CompilerInput::try_from(request.clone()).map_err(error::ErrorBadRequest)?;
    let compiler_version =
        Version::from_str(&request.compiler_version).map_err(error::ErrorBadRequest)?;

    let response = compile_and_verify_handler(
        &compilers,
        compiler_version,
        compiler_input,
        &request.creation_bytecode,
        &request.deployed_bytecode,
    )
    .await?;
    metrics::count_verify_contract(&response.status, "multi-part");
    Ok(Json(response))
}

async fn compile_and_verify_handler(
    compilers: &Compilers<VyperCompiler>,
    compiler_version: compiler::Version,
    compiler_input: CompilerInput,
    creation_tx_input: &str,
    deployed_bytecode: &str,
) -> Result<VerificationResponse, actix_web::Error> {
    let result = compilers.compile(&compiler_version, &compiler_input).await;
    let output = match result {
        Ok(output) => output,
        Err(err @ compiler::Error::VersionNotFound(_)) => return Err(error::ErrorBadRequest(err)),
        Err(err) => return Ok(VerificationResponse::err(err)),
    };
    // vyper contracts doesn't have metadata hash,
    // so modified compilation will produce the same output.
    // we can omit this step and just copy the output
    let output_modified = output.clone();
    let verifier =
        Verifier::new(creation_tx_input, deployed_bytecode).map_err(error::ErrorBadRequest)?;
    let response = match verifier.verify(output, output_modified) {
        Ok(verification_success) => {
            let verification_result =
                VerificationResult::from((compiler_input, compiler_version, verification_success));
            VerificationResponse::ok(verification_result)
        }
        Err(_) => VerificationResponse::err("No contract could be verified with provided data"),
    };
    Ok(response)
}
