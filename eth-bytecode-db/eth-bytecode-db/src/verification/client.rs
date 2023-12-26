use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Client {
    pub db_client: Arc<DatabaseConnection>,
    pub alliance_db_client: Option<Arc<DatabaseConnection>>,
    pub verifier_http_client: smart_contract_verifier_proto::http_client::Client,
}

impl Client {
    pub async fn new(
        db_client: DatabaseConnection,
        http_verifier_uri: impl Into<String>,
        max_retries: u32,
    ) -> Result<Self, anyhow::Error> {
        Self::new_arc(Arc::new(db_client), http_verifier_uri, max_retries).await
    }

    pub async fn new_arc(
        db_client: Arc<DatabaseConnection>,
        http_verifier_uri: impl Into<String>,
        max_retries: u32,
    ) -> Result<Self, anyhow::Error> {
        let verifier_http_client_config =
            smart_contract_verifier_proto::http_client::Config::new(http_verifier_uri.into())
                .with_retry_middleware(max_retries);

        let verifier_http_client =
            smart_contract_verifier_proto::http_client::Client::new(verifier_http_client_config);

        Ok(Self {
            db_client,
            alliance_db_client: None,
            verifier_http_client,
        })
    }

    pub fn with_alliance_db(self, alliance_db_client: DatabaseConnection) -> Self {
        self.with_alliance_db_arc(Arc::new(alliance_db_client))
    }

    pub fn with_alliance_db_arc(mut self, alliance_db_client: Arc<DatabaseConnection>) -> Self {
        self.alliance_db_client = Some(alliance_db_client);
        self
    }
}
