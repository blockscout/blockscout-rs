use blockscout_client::{types::IndexingStatus, v1::types::Health, Config, Error};
use url::Url;

pub async fn blockscout_indexing_status(base_url: &Url) -> Result<IndexingStatus, Error> {
    blockscout_client::indexing_status::get(&client(base_url.clone())).await
}

pub async fn blockscout_health(base_url: &Url) -> Result<Health, Error> {
    blockscout_client::v1::health::get(&client(base_url.clone())).await
}

fn client(base: Url) -> blockscout_client::Client {
    blockscout_client::Client::new(Config::new("blockscout".to_string(), base))
}
