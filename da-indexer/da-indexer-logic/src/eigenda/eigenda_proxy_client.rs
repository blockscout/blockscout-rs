use super::settings::EigendaV2ServerSettings;
use anyhow::Context;
use blockscout_display_bytes::ToHex;
use reqwest::Url;
use reqwest_middleware::{reqwest::StatusCode, ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};

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

    pub async fn request_blob(
        &self,
        base_url: Url,
        commitment: &[u8],
    ) -> Result<Option<Vec<u8>>, anyhow::Error> {
        let mut url = base_url;
        url.set_path(&format!("/get/{}", commitment.to_hex()));
        url.set_query(Some("commitment_mode=standard"));

        let response = self
            .inner_client
            .get(url)
            .send()
            .await
            .context("failed to request blob from proxy")?;

        let status_code = response.status();
        match response.error_for_status() {
            Err(_error) if status_code == StatusCode::NOT_FOUND => Ok(None),
            Err(error) => Err(error).context("proxy returned error status code"),
            Ok(response) => {
                let data = response
                    .bytes()
                    .await
                    .context("failed to read blob body from proxy")?;
                Ok(Some(data.to_vec()))
            }
        }
    }
}
