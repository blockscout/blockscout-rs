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

    let input = Input {
        compiler_version,
        compiler_input,
        creation_tx_input: &request.creation_bytecode,
        deployed_bytecode: &request.deployed_bytecode,
    };

    let response = compile_and_verify_handler(&compilers, input).await?;
    metrics::count_verify_contract(&response.status, "multi-part");
    Ok(Json(response))
}

#[derive(Debug)]
pub struct Input<'a> {
    pub compiler_version: compiler::Version,
    pub compiler_input: CompilerInput,
    pub creation_tx_input: &'a str,
    pub deployed_bytecode: &'a str,
}

async fn compile_and_verify_handler(
    compilers: &Compilers<VyperCompiler>,
    input: Input<'_>,
) -> Result<VerificationResponse, actix_web::Error> {
    let result = compilers
        .compile(&input.compiler_version, &input.compiler_input)
        .await;
    let output = match result {
        Ok(output) => output,
        Err(e) => return Ok(VerificationResponse::err(e)),
    };

    let verifier = Verifier::new(input.creation_tx_input, input.deployed_bytecode)
        .map_err(error::ErrorBadRequest)?;
    let response = match verifier.verify(output.clone(), output) {
        Ok(verification_success) => {
            let verification_result = VerificationResult::from((
                input.compiler_input,
                input.compiler_version,
                verification_success,
            ));
            VerificationResponse::ok(verification_result)
        }
        Err(errors) => VerificationResponse::err(
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join("\n"),
        ),
    };
    Ok(response)
}
