use super::smart_contract_verifier::{
    solidity_verifier_client::SolidityVerifierClient,
    sourcify_verifier_client::SourcifyVerifierClient, vyper_verifier_client::VyperVerifierClient,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tonic::transport::{Channel, Uri};

#[derive(Clone, Debug)]
pub struct Client {
    pub db_client: Arc<DatabaseConnection>,
    pub alliance_db_client: Option<Arc<DatabaseConnection>>,
    pub solidity_client: SolidityVerifierClient<Channel>,
    pub vyper_client: VyperVerifierClient<Channel>,
    pub sourcify_client: SourcifyVerifierClient<Channel>,
}

impl Client {
    pub async fn new(
        db_client: DatabaseConnection,
        verifier_uri: Uri,
    ) -> Result<Self, anyhow::Error> {
        Self::new_arc(Arc::new(db_client), verifier_uri).await
    }

    pub async fn new_arc(
        db_client: Arc<DatabaseConnection>,
        verifier_uri: Uri,
    ) -> Result<Self, anyhow::Error> {
        let channel = Channel::builder(verifier_uri)
            .connect()
            .await
            .map_err(anyhow::Error::new)?;
        let solidity_client = SolidityVerifierClient::new(channel.clone());
        let vyper_client = VyperVerifierClient::new(channel.clone());
        let sourcify_client = SourcifyVerifierClient::new(channel);

        Ok(Self {
            db_client,
            alliance_db_client: None,
            solidity_client,
            vyper_client,
            sourcify_client,
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
