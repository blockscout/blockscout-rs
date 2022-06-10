use crate::{download_cache::DownloadCache, solidity::fetcher::SvmFetcher};
use actix_web::{error, Error};
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
