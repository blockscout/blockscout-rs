use anyhow::Context;
use proxy_verifier_proto::blockscout::proxy_verifier::v1::{
    verification_response::{
        contract_validation_results, contract_verification_results::contract_verification_result,
        ContractVerificationResults, VerificationStatus,
    },
    Contract, SolidityVerifyStandardJsonRequest as VerificationRequest, VerificationResponse,
};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::BTreeMap;
use url::Url;

#[derive(Clone)]
pub struct Client {
    base_url: Url,
    request_client: ClientWithMiddleware,
}

impl Client {
    pub fn try_new(base_url: Url) -> anyhow::Result<Self> {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Ok(Self {
            base_url,
            request_client: client,
        })
    }

    pub async fn verify_contract(
        &self,
        details: entity::contract_addresses::Model,
    ) -> anyhow::Result<VerificationResponse> {
        if details.compiler != "solc" || details.language != "solidity" {
            anyhow::bail!("only solidity contracts supported at the moment")
        }

        #[derive(Clone, Debug, Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Source {
            content: String,
        }
        let sources = serde_json::from_value::<BTreeMap<String, String>>(details.sources)
            .context("sources deserialization failed")?
            .into_iter()
            .map(|(key, value)| (key, Source { content: value }))
            .collect::<BTreeMap<_, _>>();

        let input = serde_json::json!({
            "language": "Solidity",
            "sources": sources,
            "settings": details.compiler_settings
        });
        let request = VerificationRequest {
            contracts: vec![Contract {
                chain_id: details.chain_id.to_string(),
                address: hex::encode(details.address),
            }],
            compiler: details.version,
            input: serde_json::to_string(&input).context("input data serialization")?,
        };

        let url = {
            let path = "/api/v1/solidity/sources:verify-standard-json";
            let mut url = self.base_url.clone();
            url.set_path(path);
            url
        };

        let result: VerificationResponse = self
            .send_request(url, request)
            .await
            .context("sending request")?;

        match &result {
            VerificationResponse {
                verification_status:
                    Some(VerificationStatus::ContractVerificationResults(ContractVerificationResults {
                        items,
                    })),
            } => {
                if items.len() != 1 {
                    anyhow::bail!(
                        "verification results contain {} items; should be 1",
                        items.len()
                    )
                }
                let item = &items[0];
                if item.status == i32::from(contract_verification_result::Status::Success) {
                    return Ok(result.clone());
                }
                if item.status == i32::from(contract_validation_results::contract_validation_result::Status::InternalError)
                    && item.message.contains("Importing contract into blockscout failed") {
                    return Ok(result.clone())
                }
                anyhow::bail!("verification failed: {result:?}")
            }
            response => anyhow::bail!("verification failed: {response:?}"),
        }
    }

    async fn send_request<Request: Serialize, Response: DeserializeOwned>(
        &self,
        url: Url,
        request: Request,
    ) -> anyhow::Result<Response> {
        let response = self
            .request_client
            .post(url)
            .json(&request)
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
