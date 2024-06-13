use blockscout_client::{
    apis,
    apis::{health_api::HealthError, main_page_api::GetIndexingStatusError},
    models::{v1_health_check_response::V1HealthCheckResponse, IndexingStatus},
    Configuration, Error,
};
use url::Url;

pub async fn blockscout_indexing_status(
    base_url: &Url,
) -> Result<IndexingStatus, Error<GetIndexingStatusError>> {
    apis::main_page_api::get_indexing_status(&config(base_url.clone())).await
}

pub async fn blockscout_health(
    base_url: &Url,
) -> Result<V1HealthCheckResponse, Error<HealthError>> {
    apis::health_api::health(&config(base_url.clone())).await
}

fn config(base: Url) -> Configuration {
    Configuration::new(base.to_string())
}
