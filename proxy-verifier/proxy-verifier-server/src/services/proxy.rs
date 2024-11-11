use crate::{
    config::ChainsSettings,
    proto::{
        proxy_server::Proxy, Chain, GetVerificationConfigRequest, ListChainsRequest,
        ListChainsResponse, VerificationConfig,
    },
    services::{SOLIDITY_EVM_VERSIONS, VYPER_EVM_VERSIONS},
};
use async_trait::async_trait;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct ProxyService {
    /// Mapping from supported chain ids to chain names
    chains: ChainsSettings,
    eth_bytecode_db_client: Arc<eth_bytecode_db_proto::http_client::Client>,
}

impl ProxyService {
    pub fn new(
        chains_settings: ChainsSettings,
        eth_bytecode_db_client: Arc<eth_bytecode_db_proto::http_client::Client>,
    ) -> Self {
        chains_settings
            .clone()
            .inner_mut()
            .values_mut()
            .for_each(|settings| settings.sensitive_api_key = None);
        Self {
            chains: chains_settings,
            eth_bytecode_db_client,
        }
    }
}

#[async_trait]
impl Proxy for ProxyService {
    async fn list_chains(
        &self,
        _request: Request<ListChainsRequest>,
    ) -> Result<Response<ListChainsResponse>, Status> {
        let response = ListChainsResponse {
            chains: list_chains(self).await,
        };

        Ok(Response::new(response))
    }

    async fn get_verification_config(
        &self,
        _request: Request<GetVerificationConfigRequest>,
    ) -> Result<Response<VerificationConfig>, Status> {
        let solidity_compilers = super::list_compilers(
            self.eth_bytecode_db_client.as_ref(),
            eth_bytecode_db_proto::http_client::solidity_verifier_client::list_compiler_versions,
            SOLIDITY_EVM_VERSIONS,
        )
        .await?;
        let vyper_compilers = super::list_compilers(
            self.eth_bytecode_db_client.as_ref(),
            eth_bytecode_db_proto::http_client::vyper_verifier_client::list_compiler_versions,
            VYPER_EVM_VERSIONS,
        )
        .await?;

        Ok(Response::new(VerificationConfig {
            chains: list_chains(self).await,
            solidity_compilers,
            vyper_compilers,
        }))
    }
}

async fn list_chains(proxy: &ProxyService) -> Vec<Chain> {
    proxy
        .chains
        .insertion_iter()
        .map(|(id, settings)| {
            let settings = settings.clone();

            let icon_url = if let Some(icon_url) = settings.icon_url {
                icon_url.to_string()
            } else {
                let mut url = settings.api_url;
                url.set_path("assets/favicon/apple-touch-icon.png");
                url.to_string()
            };

            Chain {
                id: id.clone(),
                name: settings.name,
                icon_url,
            }
        })
        .collect()
}
