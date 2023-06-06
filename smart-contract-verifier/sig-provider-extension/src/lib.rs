use http::Uri;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use sig_provider_proto::blockscout::sig_provider::v1::{
    signature_service_client::SignatureServiceClient, CreateSignaturesRequest,
};
use smart_contract_verifier::{Middleware, SoliditySuccess, SourcifySuccess, VyperSuccess};
use std::sync::Arc;
use tonic::transport::Channel;

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde_as(as = "DisplayFromStr")]
    url: Uri,
}

pub struct SigProvider {
    inner: Arc<SigProviderImpl>,
}

impl SigProvider {
    pub async fn new(config: Config) -> Result<Self, tonic::transport::Error> {
        let connection = Channel::builder(config.url).connect().await?;
        Ok(Self {
            inner: Arc::new(SigProviderImpl { connection }),
        })
    }
}

struct SigProviderImpl {
    connection: Channel,
}

impl SigProviderImpl {
    async fn create_signatures(&self, abi: String) {
        let mut client = SignatureServiceClient::new(self.connection.clone());
        let _ = client
            .create_signatures(CreateSignaturesRequest { abi })
            .await;
    }
}

#[async_trait::async_trait]
impl Middleware<SoliditySuccess> for SigProvider {
    async fn call(&self, output: &SoliditySuccess) {
        let abi = output
            .abi
            .as_ref()
            .and_then(|abi| serde_json::to_string(abi).ok());
        if let Some(abi) = abi {
            let inner = self.inner.clone();
            tokio::spawn(async move {
                inner.create_signatures(abi).await;
            });
        }
    }
}

#[async_trait::async_trait]
impl Middleware<VyperSuccess> for SigProvider {
    async fn call(&self, output: &VyperSuccess) {
        let abi = output
            .abi
            .as_ref()
            .and_then(|abi| serde_json::to_string(abi).ok());
        if let Some(abi) = abi {
            let inner = self.inner.clone();
            tokio::spawn(async move {
                inner.create_signatures(abi).await;
            });
        }
    }
}

#[async_trait::async_trait]
impl Middleware<SourcifySuccess> for SigProvider {
    async fn call(&self, output: &SourcifySuccess) {
        let abi = serde_json::to_string(&output.abi);
        if let Ok(abi) = abi {
            let inner = self.inner.clone();
            tokio::spawn(async move {
                inner.create_signatures(abi).await;
            });
        }
    }
}
