use crate::{
    config::ChainsSettings,
    proto::{list_chains_response, proxy_server::Proxy, ListChainsRequest, ListChainsResponse},
};
use async_trait::async_trait;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct ProxyService {
    /// Mapping from supported chain ids to chain names
    chains: BTreeMap<String, String>,
}

impl ProxyService {
    pub fn new(chains_settings: &ChainsSettings) -> Self {
        let chains = chains_settings
            .inner()
            .iter()
            .map(|(chain_id, settings)| (chain_id.clone(), settings.name.clone()))
            .collect();
        Self { chains }
    }
}

#[async_trait]
impl Proxy for ProxyService {
    async fn list_chains(
        &self,
        _request: tonic::Request<ListChainsRequest>,
    ) -> Result<tonic::Response<ListChainsResponse>, tonic::Status> {
        let response = ListChainsResponse {
            chains: self
                .chains
                .clone()
                .into_iter()
                .map(|(id, name)| list_chains_response::Chain { id, name })
                .collect(),
        };

        Ok(tonic::Response::new(response))
    }
}
