use super::types::VersionsResponse;
use crate::{compiler::Compilers, solidity::SolidityCompiler};

use actix_web::{
    web::{self, Json},
    Error,
};

pub async fn get_version_list(
    compilers: web::Data<Compilers<SolidityCompiler>>,
) -> Result<Json<VersionsResponse>, Error> {
    let versions = compilers.all_versions_sorted_str();
    Ok(Json(VersionsResponse { versions }))
}
