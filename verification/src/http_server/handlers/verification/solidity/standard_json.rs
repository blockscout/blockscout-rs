use super::types::VerificationRequest;
use crate::{
    compiler::download_cache::DownloadCache,
    http_server::handlers::verification::VerificationResponse,
    solidity::github_fetcher::GithubFetcher,
};
use actix_web::{
    web::{self, Json},
    Error,
};
use ethers_solc::CompilerInput;

pub async fn verify(
    cache: web::Data<DownloadCache<GithubFetcher>>,
    params: Json<VerificationRequest<CompilerInput>>,
) -> Result<Json<VerificationResponse>, Error> {
    let params = params.into_inner();
    let output = super::compile::compile(&cache, &params.compiler_version, &params.content).await?;
    let _ = output;

    todo!("verify output")
}
