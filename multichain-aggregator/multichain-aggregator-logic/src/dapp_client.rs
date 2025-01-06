use crate::error::ServiceError;
use serde::Deserialize;
use url::Url;

pub struct DappClient {
    http: reqwest::Client,
    url: Url,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dapp {
    pub id: String,
    pub title: String,
    pub logo: String,
    pub short_description: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DappWithChainId {
    pub dapp: Dapp,
    pub chain_id: String,
}

impl DappClient {
    pub fn new(url: Url) -> Self {
        let http = reqwest::Client::new();
        Self { http, url }
    }

    pub async fn search_dapps(&self, query: &str) -> Result<Vec<DappWithChainId>, ServiceError> {
        let mut url = self.url.clone();
        url.set_path("/api/v1/marketplace/dapps:search");
        url.query_pairs_mut().append_pair("query", query);

        self.http
            .get(url)
            .send()
            .await
            .map_err(|e| ServiceError::Internal(e.into()))?
            .json::<Vec<DappWithChainId>>()
            .await
            .map_err(|e| ServiceError::Internal(e.into()))
    }
}
