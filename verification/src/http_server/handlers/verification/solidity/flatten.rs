use super::types::{FlattenedSource, VerificationRequest};
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
    params: Json<VerificationRequest<FlattenedSource>>,
) -> Result<Json<VerificationResponse>, Error> {
    let params = params.into_inner();

    let input = CompilerInput::try_from(params.content).map_err(error::ErrorBadRequest)?;
    let compiler_version =
        CompilerVersion::from_str(&params.compiler_version).map_err(error::ErrorBadRequest)?;
    let output = compilers.compile(&compiler_version, &input).await;
    let _ = output;

    todo!("verify output")
}
