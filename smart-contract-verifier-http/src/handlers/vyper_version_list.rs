use crate::versions::VersionsResponse;
use actix_web::{
    web::{self, Json},
    Error,
};
use smart_contract_verifier::VyperClient;

pub async fn get_version_list(
    client: web::Data<VyperClient>,
) -> Result<Json<VersionsResponse>, Error> {
    let versions = client.compilers().all_versions_sorted_str();
    Ok(Json(VersionsResponse { versions }))
}
