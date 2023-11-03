use ethers::types::TxHash;
use futures::StreamExt;
use reqwest::StatusCode;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{de::DeserializeOwned, Deserialize};
use std::sync::Arc;
use tracing::instrument;

const MAX_REQUESTS_BATCH: usize = 5;

#[derive(Debug, Clone)]
pub struct BlockscoutClient {
    url: url::Url,
    inner: ClientWithMiddleware,
}

impl BlockscoutClient {
    pub fn new(url: url::Url) -> Self {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();
        Self { url, inner: client }
    }

    pub fn url(&self) -> &url::Url {
        &self.url
    }
}

#[derive(Debug, Deserialize)]
pub struct TransactionFrom {
    pub hash: ethers::types::Address,
}

#[derive(Debug, Deserialize)]
pub struct Transaction {
    pub timestamp: String,
    pub method: Option<String>,
    pub from: TransactionFrom,
    pub hash: ethers::types::TxHash,
    pub block: i64,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub message: String,
}

#[derive(Debug)]
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

impl BlockscoutClient {
    #[instrument(name = "blockscout_api:transaction", skip_all, err, level = "debug")]
    pub async fn transaction(
        &self,
        transaction_hash: &ethers::types::TxHash,
    ) -> reqwest_middleware::Result<Response<Transaction>> {
        let response = self
            .inner
            .get(
                self.url
                    .join(&format!("/api/v2/transactions/{transaction_hash:#x}"))
                    .unwrap(),
            )
            .send()
            .await?;
        Response::try_from_reqwest_response(response).await
    }

    pub async fn transactions_batch(
        self: Arc<Self>,
        transaction_hashes: Vec<&ethers::types::TxHash>,
    ) -> reqwest_middleware::Result<Vec<(TxHash, Response<Transaction>)>> {
        let fetches = futures::stream::iter(transaction_hashes.into_iter().cloned().map(|hash| {
            let client = self.clone();
            async move {
                let result = client.transaction(&hash).await;
                result.map(|r| (TxHash::clone(&hash), r))
            }
        }))
        .buffer_unordered(MAX_REQUESTS_BATCH)
        .collect::<Vec<_>>();
        let result = fetches.await.into_iter().collect::<Result<Vec<_>, _>>()?;
        Ok(result)
    }
}
