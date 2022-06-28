use super::types::VerificationRequest;
use crate::{
    compiler::{version::CompilerVersion, Compilers},
    http_server::handlers::verification::{solidity::types::StandardJson, VerificationResponse},
    solidity::compiler_fetcher::CompilerFetcher,
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
    let compiler_version =
        CompilerVersion::from_str(&params.compiler_version).map_err(error::ErrorBadRequest)?;
    let output = compilers
        .compile(&compiler_version, &params.content.into())
        .await;
    let _ = output;

    todo!("verify output")
}
