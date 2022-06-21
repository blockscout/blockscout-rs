use super::types::VerificationRequest;
use crate::{
    compiler::{version::CompilerVersion, Compilers},
    http_server::handlers::verification::VerificationResponse,
    solidity::compiler_fetcher::CompilerFetcher,
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
    params: Json<VerificationRequest<CompilerInput>>,
) -> Result<Json<VerificationResponse>, Error> {
    let params = params.into_inner();
    let compiler_version =
        CompilerVersion::from_str(&params.compiler_version).map_err(error::ErrorBadRequest)?;
    let output = compilers.compile(&compiler_version, &params.content).await;
    let _ = output;

    todo!("verify output")
}
