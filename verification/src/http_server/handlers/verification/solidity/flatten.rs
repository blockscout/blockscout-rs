use super::{
    compile::compile,
    types::{FlattenedSource, VerificationRequest},
};
use crate::{
    compiler::download_cache::DownloadCache,
    http_server::handlers::verification::VerificationResponse,
    solidity::github_fetcher::GithubFetcher,
};
use actix_web::{
    error,
    web::{self, Json},
    Error,
};
use ethers_solc::CompilerInput;

pub async fn verify(
    cache: web::Data<DownloadCache<GithubFetcher>>,
    params: Json<VerificationRequest<FlattenedSource>>,
) -> Result<Json<VerificationResponse>, Error> {
    let params = params.into_inner();

    let input = CompilerInput::try_from(params.content).map_err(error::ErrorBadRequest)?;
    let output = compile(&cache, &params.compiler_version, &input).await?;
    let _ = output;

    todo!("verify output")
}
