use super::types::{MultiSource, VerificationRequest};
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
    params: Json<VerificationRequest<MultiSource>>,
) -> Result<Json<VerificationResponse>, Error> {
    let params = params.into_inner();

    let input = CompilerInput::try_from(params.content).map_err(error::ErrorBadRequest)?;
    let compiler_version =
        CompilerVersion::from_str(&params.compiler_version).map_err(error::ErrorBadRequest)?;
    let output = compilers
        .compile(&compiler_version, &input)
        .await
        .map_err(error::ErrorInternalServerError)?;

    log::info!("{:?}", output.contracts["main.sol"][&params.contract_name]);
    todo!("verify output")
}
