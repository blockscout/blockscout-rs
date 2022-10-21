use smart_contract_verifier::{Compilers, SolidityCompiler};

use actix_web::{
    web::{self, Json},
    Error,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct VersionsResponse {
    pub versions: Vec<String>,
}

pub async fn get_version_list(
    compilers: web::Data<Compilers<SolidityCompiler>>,
) -> Result<Json<VersionsResponse>, Error> {
    let versions = compilers.all_versions_sorted_str();
    Ok(Json(VersionsResponse { versions }))
}
