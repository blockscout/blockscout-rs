use super::settings::EigendaV2ServerSettings;
use anyhow::Context;
use blockscout_display_bytes::ToHex;
use reqwest::Url;
use reqwest_middleware::{reqwest::StatusCode, ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("blob not found")]
    NotFound,
    #[error("proxy returned error: status_code={status_code}, error={error}")]
    ProxyError {
        status_code: StatusCode,
        error: String,
    },
    #[error("{0:#?}")]
    Internal(#[from] anyhow::Error),
}

#[derive(Clone)]
pub struct Client {
    inner_client: ClientWithMiddleware,
}

impl Client {
    pub fn new(config: EigendaV2ServerSettings) -> Self {
        let retry_policy =
            ExponentialBackoff::builder().build_with_max_retries(config.proxy_request_retries);
        let inner_client = ClientBuilder::new(
            reqwest::Client::builder()
                .timeout(config.proxy_request_timeout)
                .build()
                .expect("cannot initialize eigenda_proxy_client"),
        )
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();

        Client { inner_client }
    }

    pub async fn request_blob(&self, base_url: Url, commitment: &[u8]) -> Result<Vec<u8>, Error> {
        let mut url = base_url;
        url.set_path(&format!("/get/{}", commitment.to_hex()));
        url.set_query(Some("commitment_mode=standard"));

        let response = self
            .inner_client
            .get(url)
            .send()
            .await
            .context("failed to request blob from proxy")?;

        match response.status() {
            StatusCode::OK => {
                let data = response
                    .bytes()
                    .await
                    .context("failed to read blob body from proxy")?;
                Ok(data.to_vec())
            }
            StatusCode::NOT_FOUND => Err(Error::NotFound),
            status_code => {
                let error = response
                    .text()
                    .await
                    .context("failed to read body of error response")?;

                // For some reason proxy returns 500 with specific body text when the requested blob is not found.
                // Here we process such responses as we would like to return 404 for such blobs.
                if status_code.is_server_error() && error.contains("all retrievers failed") {
                    return Err(Error::NotFound);
                }

                Err(Error::ProxyError { status_code, error })
            }
        }
    }
}
