use cached::proc_macro::cached;
use ethers::types::TxHash;
use futures::StreamExt;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::{collections::HashMap, fmt::Debug, sync::Arc};
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct BlockscoutClient {
    url: url::Url,
    inner: ClientWithMiddleware,
    max_concurrent_requests: usize,
}

impl BlockscoutClient {
    pub fn new(url: url::Url, max_concurrent_requests: usize, timeout_seconds: u64) -> Self {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client = ClientBuilder::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(timeout_seconds))
                .build()
                .expect("valid client"),
        )
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();
        Self {
            url,
            inner: client,
            max_concurrent_requests,
        }
    }

    pub fn url(&self) -> &url::Url {
        &self.url
    }
}

use reqwest::StatusCode;
use serde::{de::DeserializeOwned, Deserialize};
#[derive(Debug, Clone, Deserialize)]
pub struct TransactionFrom {
    pub hash: ethers::types::Address,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Transaction {
    pub timestamp: String,
    pub method: Option<String>,
    pub from: TransactionFrom,
    pub hash: ethers::types::TxHash,
    pub block: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum Response<T> {
    Ok(T),
    NotFound(String),
    Error(String),
}

impl<T> Response<T>
where
    T: DeserializeOwned,
{
    async fn try_from_reqwest_response(
        response: reqwest::Response,
    ) -> reqwest_middleware::Result<Self> {
        let response = match response.status() {
            StatusCode::OK => Response::Ok(response.json().await?),
            StatusCode::NOT_FOUND => Response::NotFound(response.text().await?),
            _ => Response::Error(response.text().await?),
        };
        Ok(response)
    }
}

#[cached(
    key = "String",
    convert = r#"{ 
        let url = client.url();
        format!("{url}/tx/{transaction_hash:#}")
    }"#,
    result = true,
    time = 86_400, // 24 * 60 * 60 seconds
    size = 50_000,
    sync_writes = true,
)]
pub async fn cached_transaction(
    client: &BlockscoutClient,
    transaction_hash: &ethers::types::TxHash,
) -> reqwest_middleware::Result<Response<Transaction>> {
    let response = client
        .inner
        .get(
            client
                .url
                .join(&format!("/api/v2/transactions/{transaction_hash:#x}"))
                .unwrap(),
        )
        .send()
        .await?;
    Response::try_from_reqwest_response(response).await
}

impl BlockscoutClient {
    #[instrument(name = "blockscout_api:transaction", skip(self), err, level = "debug")]
    pub async fn transaction(
        &self,
        transaction_hash: &ethers::types::TxHash,
    ) -> reqwest_middleware::Result<Response<Transaction>> {
        cached_transaction(self, transaction_hash).await
    }

    pub async fn transactions_batch(
        self: Arc<Self>,
        transaction_hashes: impl IntoIterator<Item = ethers::types::TxHash>,
    ) -> reqwest_middleware::Result<HashMap<TxHash, Response<Transaction>>> {
        let fetches = futures::stream::iter(transaction_hashes.into_iter().map(|hash| {
            let client = self.clone();
            async move {
                let result = client.transaction(&hash).await;
                result.map(|r| (TxHash::clone(&hash), r))
            }
        }))
        .buffer_unordered(self.max_concurrent_requests)
        .collect::<Vec<_>>();
        let result = fetches
            .await
            .into_iter()
            .collect::<Result<HashMap<_, _>, _>>()?;
        Ok(result)
    }
}
