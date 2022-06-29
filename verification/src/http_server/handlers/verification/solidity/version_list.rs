use super::types::VersionsResponse;
use crate::{
    compiler::{Compilers, VersionList},
    solidity::CompilerFetcher,
};

use actix_web::{
    web::{self, Json},
    Error,
};

pub async fn get_version_list(
    compilers: web::Data<Compilers<CompilerFetcher>>,
) -> Result<Json<VersionsResponse>, Error> {
    let mut versions = compilers.all_versions().await;
    // sort in descending order
    versions.sort_by(|x, y| x.cmp(y).reverse());
    let versions = versions.into_iter().map(|v| v.to_string()).collect();

    Ok(Json(VersionsResponse { versions }))
}
