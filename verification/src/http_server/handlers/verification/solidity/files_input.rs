use super::types::{SourcesInput, VerificationRequest};
use crate::{
    compiler::{CompilerVersion, Compilers},
    http_server::handlers::verification::VerificationResponse,
    solidity::CompilerFetcher,
};
use actix_web::{
    error,
    web::{self, Json},
    Error,
};
use ethers_solc::CompilerInput;
use std::str::FromStr;

pub async fn verify(
    compilers: web::Data<Compilers<CompilerFetcher>>,
    params: Json<VerificationRequest<SourcesInput>>,
) -> Result<Json<VerificationResponse>, Error> {
    let params = params.into_inner();

    let input = CompilerInput::try_from(params.content).map_err(error::ErrorBadRequest)?;
    let compiler_version =
        CompilerVersion::from_str(&params.compiler_version).map_err(error::ErrorBadRequest)?;
    let _output = compilers
        .compile(&compiler_version, &input)
        .await
        .map_err(error::ErrorInternalServerError)?;

    todo!("verify output")
}
