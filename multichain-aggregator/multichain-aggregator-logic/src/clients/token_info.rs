use crate::{error::ServiceError, ChainId};
use serde::Deserialize;
use url::Url;

pub struct TokenInfoClient {
    http: reqwest::Client,
    url: Url,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenInfo {
    pub token_address: String,
    pub chain_id: String,
    pub icon_url: String,
    pub token_name: Option<String>,
    pub token_symbol: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TokenInfoSearchResponse {
    pub token_infos: Vec<TokenInfo>,
    pub next_page_params: Option<String>,
}

impl TokenInfoClient {
    pub fn new(url: Url) -> Self {
        let http = reqwest::Client::new();
        Self { http, url }
    }

    pub async fn search_tokens(
        &self,
        query: &str,
        chain_id: Option<ChainId>,
        page_size: Option<u32>,
        page_token: Option<String>,
    ) -> Result<TokenInfoSearchResponse, ServiceError> {
        let mut url = self.url.clone();
        url.set_path("/api/v1/token-infos:search");
        url.query_pairs_mut().append_pair("query", query);

        if let Some(chain_id) = chain_id {
            url.query_pairs_mut()
                .append_pair("chain_id", &chain_id.to_string());
        }
        if let Some(page_size) = page_size {
            url.query_pairs_mut()
                .append_pair("page_size", &page_size.to_string());
        }
        if let Some(page_token) = page_token {
            url.query_pairs_mut().append_pair("page_token", &page_token);
        }

        self.http
            .get(url)
            .send()
            .await
            .map_err(|e| ServiceError::Internal(e.into()))?
            .json::<TokenInfoSearchResponse>()
            .await
            .map_err(|e| ServiceError::Internal(e.into()))
    }
}
