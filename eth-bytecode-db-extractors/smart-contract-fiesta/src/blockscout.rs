use anyhow::Context;
use blockscout_display_bytes::Bytes;
use governor::{Quota, RateLimiter};
use reqwest::Response;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_rate_limiter::RateLimiterMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::Deserialize;
use std::{num::NonZeroU32, str::FromStr, time::Duration};
use url::Url;

#[derive(Clone)]
pub struct Client {
    blockscout_base_url: Url,
    etherscan_base_url: Url,
    etherscan_api_key: String,
    request_client: ClientWithMiddleware,
}

impl Client {
    pub fn try_new(
        blockscout_base_url: String,
        etherscan_base_url: String,
        etherscan_api_key: String,
        etherscan_limit_requests_per_second: u32,
    ) -> anyhow::Result<Self> {
        let blockscout_base_url =
            Url::from_str(&blockscout_base_url).context("invalid blockscout base url")?;
        let etherscan_base_url =
            Url::from_str(&etherscan_base_url).context("invalid etherscan base url")?;
        let max_burst = NonZeroU32::new(etherscan_limit_requests_per_second)
            .ok_or_else(|| anyhow::anyhow!("invalid etherscan limit requests per second"))?;

        let rate_limiter = RateLimiter::direct(Quota::per_second(max_burst));

        let retry_policy =
            ExponentialBackoff::builder().build_with_total_retry_duration(Duration::from_secs(20));
        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .with(RateLimiterMiddleware::new(rate_limiter))
            .build();

        Ok(Self {
            blockscout_base_url,
            etherscan_base_url,
            etherscan_api_key,
            request_client: client,
        })
    }

    pub async fn get_transaction_input(&self, transaction_hash: Bytes) -> anyhow::Result<Bytes> {
        #[derive(Deserialize, Debug)]
        struct Result {
            input: Bytes,
        }

        #[derive(Deserialize, Debug)]
        struct Response {
            result: Result,
        }

        let url = {
            let mut url = self.etherscan_base_url.clone();
            url.set_path("api");
            url.set_query(Some(&format!(
                "module=proxy&action=eth_getTransactionByHash&txhash={transaction_hash}&apikey={}",
                self.etherscan_api_key
            )));
            url
        };

        let response = self
            .send_request(url)
            .await?
            .json::<Response>()
            .await
            .context("deserializing response failed")?;

        Ok(response.result.input)
    }

    pub async fn get_contract_creation_transaction(
        &self,
        contract_address: Bytes,
    ) -> anyhow::Result<Bytes> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Result {
            tx_hash: Bytes,
        }

        #[derive(Deserialize)]
        struct Response {
            result: Vec<Result>,
        }

        let url = {
            let mut url = self.etherscan_base_url.clone();
            url.set_path("api");
            url.set_query(Some(&format!(
                "module=contract&action=getcontractcreation&contractaddresses={contract_address}&apikey={}",
                self.etherscan_api_key
            )));
            url
        };
        let response = self
            .send_request(url)
            .await?
            .json::<Response>()
            .await
            .context("deserializing response failed")?;

        let tx_hash = response
            .result
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("result is empty"))?
            .tx_hash
            .clone();

        Ok(tx_hash)
    }

    pub async fn get_transaction_input_blockscout(
        &self,
        transaction_hash: Bytes,
    ) -> anyhow::Result<Bytes> {
        #[derive(Deserialize)]
        struct Result {
            input: Bytes,
        }

        #[derive(Deserialize)]
        struct Response {
            result: Result,
        }

        let url = {
            let path = format!("{}/api", self.blockscout_base_url.path());
            let mut url = self.blockscout_base_url.clone();
            url.set_path(&path);
            url.set_query(Some(&format!(
                "module=transaction&action=gettxinfo&txhash={transaction_hash}"
            )));
            url
        };
        let response = self
            .send_request(url)
            .await?
            .json::<Response>()
            .await
            .context("deserializing response failed")?;

        Ok(response.result.input)
    }

    pub async fn get_contract_creation_transaction_blockscout(
        &self,
        contract_address: Bytes,
    ) -> anyhow::Result<Bytes> {
        let address_page_url = {
            let path = format!(
                "{}/address/{}",
                self.blockscout_base_url.path(),
                contract_address
            );
            let mut address_page_url = self.blockscout_base_url.clone();
            address_page_url.set_path(&path);
            address_page_url
        };

        let address_page = self.send_request(address_page_url).await?;

        let document = scraper::Html::parse_document(
            &address_page
                .text()
                .await
                .context("parsing page as a string failed")?,
        );

        let selector = scraper::Selector::parse("dd").unwrap();
        for elem in document.select(&selector) {
            for node in elem.descendants() {
                if let Some(elem) = node.value().as_element() {
                    match elem.attr("data-test") {
                        Some("address_contract_creator") => {
                            for node in node.descendants() {
                                if let Some(elem) = node.value().as_element() {
                                    match elem.attr("data-test") {
                                        Some("transaction_hash_link") => {
                                            let href = elem.attr("href").ok_or_else(|| {
                                                anyhow::anyhow!(
                                                    "missed href for the transaction element"
                                                )
                                            })?;
                                            let tx_hash = href.split('/').last().ok_or_else(
                                                || anyhow::anyhow!("missed transaction hash in the transaction href element")
                                            )?;
                                            return Bytes::from_str(tx_hash).context(
                                                "converting transaction hash as into bytes",
                                            );
                                        }
                                        _ => continue,
                                    }
                                }
                            }
                        }
                        _ => continue,
                    }
                }
            }
        }

        Err(anyhow::anyhow!(
            "creation transaction hash was not found on the page"
        ))
    }

    async fn send_request(&self, url: Url) -> anyhow::Result<Response> {
        let response = self
            .request_client
            .get(url)
            .send()
            .await
            .context("sending request failed")?;

        // Continue only in case if request results is success
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Invalid status code get as a result: {}",
                response.status()
            ));
        }

        Ok(response)
    }
}
