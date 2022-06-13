use std::str::FromStr;

use crate::compiler::{download_cache::DownloadCache, fetcher::Fetcher, version::CompilerVersion};
use actix_web::{error, Error};
use ethers_solc::{CompilerInput, CompilerOutput, Solc};

pub async fn compile<T: Fetcher>(
    cache: &DownloadCache<T>,
    compiler_version: &str,
    input: &CompilerInput,
) -> Result<CompilerOutput, Error>
where
    <T as Fetcher>::Error: 'static + std::fmt::Debug + std::fmt::Display,
{
    let ver = CompilerVersion::from_str(compiler_version).map_err(error::ErrorBadRequest)?;
    let solc_path = cache
        .get(&ver)
        .await
        .map_err(error::ErrorInternalServerError)?;
    let solc = Solc::from(solc_path);
    solc.compile(&input)
        .map_err(error::ErrorInternalServerError)
}
