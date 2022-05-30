use super::types::{FlattenedSource, VerificationRequest, VerificationResponse};
use crate::{download_cache::DownloadCache, solidity::fetcher::SvmFetcher};
use actix_web::{
    error,
    web::{self, Json},
    Error,
};
use ethers_solc::{CompilerInput, CompilerOutput, Solc};
use semver::Version;

pub async fn compile(
    cache: &DownloadCache<SvmFetcher>,
    compiler_version: &str,
    input: &CompilerInput,
) -> Result<CompilerOutput, Error> {
    let ver = Version::parse(compiler_version).map_err(error::ErrorBadRequest)?;
    let solc_path = cache
        .get(&ver)
        .await
        .map_err(error::ErrorInternalServerError)?;
    let solc = Solc::from(solc_path);
    solc.compile(&input)
        .map_err(error::ErrorInternalServerError)
}

pub async fn verify(
    cache: web::Data<DownloadCache<SvmFetcher>>,
    params: Json<VerificationRequest<FlattenedSource>>,
) -> Result<Json<VerificationResponse>, Error> {
    let params = params.into_inner();

    let input = CompilerInput::try_from(params.content).map_err(error::ErrorBadRequest)?;
    let output = compile(&cache, &params.compiler_version, &input).await?;
    // TODO: verify output
    let _ = output;

    Ok(Json(VerificationResponse { verified: true }))
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn compile() {}
}
