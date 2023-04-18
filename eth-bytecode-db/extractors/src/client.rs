use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use url::Url;

#[derive(Clone)]
pub struct Client {
    pub db: Arc<DatabaseConnection>,
    pub blockscout_client: ClientWithMiddleware,
    pub blockscout_url: Url,
}

impl Client {
    pub fn new(db: DatabaseConnection, blockscout_api: Url) -> Self {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let reqwest_client = reqwest::Client::builder()
            .build()
            .expect("Client build failed");
        let blockscout_client = reqwest_middleware::ClientBuilder::new(reqwest_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();
        Self {
            db: Arc::new(db),
            blockscout_client,
            blockscout_url: blockscout_api,
        }
    }
}
