use crate::{
    compiler::Compilers, http_server::handlers::verification::solidity::types::VersionsResponse,
    vyper::VyperCompiler,
};

use actix_web::{
    web::{self, Json},
    Error,
};

pub async fn get_version_list(
    compilers: web::Data<Compilers<VyperCompiler>>,
) -> Result<Json<VersionsResponse>, Error> {
    let versions = compilers.all_versions_sorted_str();
    Ok(Json(VersionsResponse { versions }))
}
