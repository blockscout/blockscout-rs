use super::{
    handlers::compile_and_verify,
    types::{FlattenedSource, VerificationRequest},
};
use crate::{
    compiler::{version::CompilerVersion, Compilers},
    http_server::handlers::verification::{
        solidity::handlers::{CompileAndVerifyError, CompileAndVerifyInput},
        VerificationResponse,
    },
    solidity::github_fetcher::GithubFetcher,
    VerificationResult,
};
use actix_web::{
    error,
    web::{self, Json},
    Error,
};
use ethers_solc::CompilerInput;
use std::str::FromStr;

pub async fn verify(
    compilers: web::Data<Compilers<GithubFetcher>>,
    params: Json<VerificationRequest<FlattenedSource>>,
) -> Result<Json<VerificationResponse>, Error> {
    let params = params.into_inner();

    let compiler_input = CompilerInput::try_from(params.content).map_err(error::ErrorBadRequest)?;
    let compiler_version =
        CompilerVersion::from_str(&params.compiler_version).map_err(error::ErrorBadRequest)?;
    let input = CompileAndVerifyInput {
        compiler_version: &compiler_version,
        compiler_input: &compiler_input,
        creation_tx_input: &params.creation_bytecode,
        deployed_bytecode: &params.deployed_bytecode,
    };
    match compile_and_verify(&compilers, &input).await {
        Ok(verification_success) => {
            let verification_result =
                VerificationResult::from((compiler_input, compiler_version, verification_success));
            Ok(Json(VerificationResponse::ok(verification_result)))
        }
        Err(CompileAndVerifyError::VerifierInitialization(err)) => Err(error::ErrorBadRequest(err)),
        Err(err) => Ok(Json(VerificationResponse::err(err))),
    }
}
