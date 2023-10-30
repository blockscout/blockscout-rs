use anyhow::Context;
use blockscout_display_bytes::Bytes;
use governor::{Quota, RateLimiter};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_rate_limiter::RateLimiterMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{de::DeserializeOwned, Deserialize};
use std::{collections::BTreeMap, num::NonZeroU32, str::FromStr};
use url::Url;

#[derive(Clone)]
pub struct Client {
    blockscout_base_url: Url,
    request_client: ClientWithMiddleware,
}

pub struct ContractDetails {
    pub creation_code: Option<Vec<u8>>,
    pub runtime_code: Vec<u8>,

    pub transaction_hash: Vec<u8>,
    pub block_number: u64,
    pub transaction_index: Option<u64>,
    pub deployer: Vec<u8>,

    pub sources: serde_json::Value,
    pub settings: Option<serde_json::Value>,

    pub verified_via_sourcify: bool,
    pub optimization_enabled: Option<bool>,
    pub optimization_runs: Option<i64>,
    pub evm_version: Option<String>,
    pub libraries: Option<serde_json::Value>,
}

impl Client {
    pub fn try_new(
        blockscout_base_url: String,
        limit_requests_per_second: u32,
    ) -> anyhow::Result<Self> {
        let blockscout_base_url =
            Url::from_str(&blockscout_base_url).context("invalid blockscout base url")?;
        let max_burst = NonZeroU32::new(limit_requests_per_second)
            .ok_or_else(|| anyhow::anyhow!("invalid limit requests per second"))?;

        let rate_limiter = RateLimiter::direct(Quota::per_second(max_burst));

        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .with(RateLimiterMiddleware::new(rate_limiter))
            .build();

        Ok(Self {
            blockscout_base_url,
            request_client: client,
        })
    }

    pub async fn get_verified_contracts(
        &self,
    ) -> anyhow::Result<verified_contracts::VerifiedContractsIterator> {
        verified_contracts::VerifiedContractsIterator::new(self.clone()).await
    }

    pub async fn get_contract_details(
        &self,
        contract_address: Bytes,
    ) -> anyhow::Result<ContractDetails> {
        // creation_code, runtime_code, transaction_hash, block_number, transaction_index, deployer

        let smart_contract_details =
            smart_contracts::retrieve_smart_contract_details(self, contract_address.clone())
                .await
                .context("get smart contract details failed")?;
        let address_details = addresses::retrieve_address_details(self, contract_address)
            .await
            .context("get address details failed")?;
        let transaction_details = transactions::retrieve_transaction_details(
            self,
            address_details.creation_tx_hash.clone(),
        )
        .await
        .context("get transaction details failed")?;

        let sources = smart_contracts::retrieve_sources(&smart_contract_details);

        let libraries = smart_contracts::parse_external_libraries(
            smart_contract_details.external_libraries.clone(),
        );

        Ok(ContractDetails {
            creation_code: smart_contract_details.creation_bytecode.map(|v| v.to_vec()),
            runtime_code: smart_contract_details.deployed_bytecode.to_vec(),

            transaction_hash: address_details.creation_tx_hash.to_vec(),
            block_number: transaction_details.block,
            transaction_index: None,
            deployer: address_details.creator_address_hash.to_vec(),

            sources,
            settings: smart_contract_details.compiler_settings,

            verified_via_sourcify: smart_contract_details
                .is_verified_via_sourcify
                .unwrap_or_default(),
            optimization_enabled: smart_contract_details.optimization_enabled,
            optimization_runs: smart_contract_details.optimization_runs,
            evm_version: smart_contract_details.evm_version,
            libraries,
        })
    }

    async fn send_request<Response: DeserializeOwned>(&self, url: Url) -> anyhow::Result<Response> {
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

        let result = response
            .text()
            .await
            .context("deserializing response into string failed")?;
        let jd = &mut serde_json::Deserializer::from_str(&result);
        serde_path_to_error::deserialize(jd).context("deserializing response failed")
    }
}

mod verified_contracts {
    use super::*;

    pub struct VerifiedContract {
        pub address: Vec<u8>,
        pub verified_at: chrono::DateTime<chrono::FixedOffset>,
        pub language: String,
        pub compiler_version: String,
    }

    /// Used as a return type from [`Client::get_verified_contracts`].
    /// Does not implement an `Iterator` trait due to internal async calls,
    /// but provides a `next` function which could be used by the caller.
    /// On each `next` call the request to blockscout server is made,
    /// which returns the next 50 elements.
    pub struct VerifiedContractsIterator {
        client: Client,
        url: Url,
        next_page_params: Option<NextPageParams>,
        items: Vec<VerifiedContract>,
    }

    impl VerifiedContractsIterator {
        pub async fn new(client: Client) -> anyhow::Result<Self> {
            let url = {
                let path = "/api/v2/smart-contracts";
                let mut url = client.blockscout_base_url.clone();
                url.set_path(path);
                url
            };

            let response = Self::load_next_page(&client, url.clone(), 0, None).await?;

            Ok(Self {
                client,
                url,
                next_page_params: response.next_page_params,
                items: Self::process_response_items(response.items),
            })
        }

        /// Returns the next 50 verified contracts. The number of contracts may be less
        /// if that is the last page. The next iteration will return `None` in that case.
        pub async fn next_page(&mut self) -> anyhow::Result<Option<Vec<VerifiedContract>>> {
            if self.items.is_empty() {
                return Ok(None);
            }

            let items = self.items.drain(..).collect::<Vec<_>>();

            if let Some(next_page_params) = &self.next_page_params {
                let response = Self::load_next_page(
                    &self.client,
                    self.url.clone(),
                    next_page_params.items_count,
                    Some(next_page_params.smart_contract_id),
                )
                .await?;
                self.next_page_params = response.next_page_params;
                self.items = Self::process_response_items(response.items);
            }

            Ok(Some(items))
        }

        pub fn smart_contract_id(&self) -> Option<usize> {
            self.next_page_params
                .as_ref()
                .map(|params| params.smart_contract_id)
        }

        pub fn items_count(&self) -> Option<usize> {
            self.next_page_params
                .as_ref()
                .map(|params| params.items_count)
        }

        async fn load_next_page(
            client: &Client,
            mut url: Url,
            items_count: usize,
            smart_contract_id: Option<usize>,
        ) -> anyhow::Result<Response> {
            let query = {
                let mut query = format!("items_count={items_count}");
                if let Some(smart_contract_id) = smart_contract_id {
                    query.push_str(&format!("&smart_contract_id={smart_contract_id}"));
                }
                query
            };
            url.set_query(Some(&query));

            client.send_request(url).await
        }

        fn process_response_items(items: Vec<Item>) -> Vec<VerifiedContract> {
            items
                .into_iter()
                .map(|item| VerifiedContract {
                    address: item.address.hash.to_vec(),
                    verified_at: item.verified_at,
                    language: item.language,
                    compiler_version: item.compiler_version,
                })
                .collect()
        }
    }

    #[derive(Debug, Deserialize)]
    struct Response {
        items: Vec<Item>,
        next_page_params: Option<NextPageParams>,
    }

    #[derive(Debug, Deserialize)]
    struct NextPageParams {
        items_count: usize,
        smart_contract_id: usize,
    }

    #[derive(Debug, Deserialize)]
    struct Item {
        address: Address,
        language: String,
        verified_at: chrono::DateTime<chrono::FixedOffset>,
        compiler_version: String,
    }

    #[derive(Debug, Deserialize)]
    struct Address {
        hash: Bytes,
    }
}

mod smart_contracts {
    use super::*;
    use serde::Serialize;

    pub async fn retrieve_smart_contract_details(
        client: &Client,
        address: Bytes,
    ) -> anyhow::Result<Response> {
        let url = {
            let path = format!("/api/v2/smart-contracts/{address}");
            let mut url = client.blockscout_base_url.clone();
            url.set_path(&path);
            url
        };

        client.send_request(url).await
    }

    pub fn retrieve_sources(response: &Response) -> serde_json::Value {
        #[derive(Debug, Serialize)]
        struct Source<'a> {
            content: &'a str,
        }

        let mut sources = BTreeMap::new();
        sources.insert(
            response.file_path.as_deref().unwrap_or(".sol"),
            Source {
                content: response.source_code.as_str(),
            },
        );
        if let Some(additional_sources) = response.additional_sources.as_ref() {
            for additional_source in additional_sources {
                sources.insert(
                    additional_source.file_path.as_str(),
                    Source {
                        content: additional_source.source_code.as_str(),
                    },
                );
            }
        }
        serde_json::to_value(sources).unwrap()
    }

    pub fn parse_external_libraries(libraries: Vec<ExternalLibrary>) -> Option<serde_json::Value> {
        if libraries.is_empty() {
            return None;
        }

        Some(serde_json::to_value(libraries).unwrap())
    }

    #[derive(Debug, Deserialize)]
    pub struct Response {
        pub verified_at: chrono::DateTime<chrono::FixedOffset>,

        pub compiler_version: String,
        pub source_code: String,
        pub file_path: Option<String>,
        pub compiler_settings: Option<serde_json::Value>,
        pub additional_sources: Option<Vec<AdditionalSource>>,
        pub deployed_bytecode: Bytes,
        pub creation_bytecode: Option<Bytes>,

        // pub is_vyper_contract: Option<bool>,
        pub is_verified_via_sourcify: Option<bool>,
        pub optimization_enabled: Option<bool>,
        pub optimization_runs: Option<i64>,
        pub evm_version: Option<String>,
        pub external_libraries: Vec<ExternalLibrary>,
    }

    #[derive(Debug, Deserialize)]
    pub struct AdditionalSource {
        file_path: String,
        source_code: String,
    }

    #[derive(Debug, Clone, Deserialize, Serialize)]
    pub struct ExternalLibrary {
        name: String,
        address_hash: Bytes,
    }
}

mod addresses {
    use super::*;

    pub async fn retrieve_address_details(
        client: &Client,
        address: Bytes,
    ) -> anyhow::Result<Response> {
        let url = {
            let path = format!("/api/v2/addresses/{address}");
            let mut url = client.blockscout_base_url.clone();
            url.set_path(&path);
            url
        };

        client.send_request(url).await
    }

    #[derive(Debug, Deserialize)]
    pub struct Response {
        pub creator_address_hash: Bytes,
        pub creation_tx_hash: Bytes,
    }
}

mod transactions {
    use super::*;

    pub async fn retrieve_transaction_details(
        client: &Client,
        transaction_hash: Bytes,
    ) -> anyhow::Result<Response> {
        let url = {
            let path = format!("/api/v2/transactions/{transaction_hash}");
            let mut url = client.blockscout_base_url.clone();
            url.set_path(&path);
            url
        };

        client.send_request(url).await
    }

    #[derive(Debug, Deserialize)]
    pub struct Response {
        pub block: u64,
    }
}
