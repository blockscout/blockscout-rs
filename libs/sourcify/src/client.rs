use crate::{
    types::{
        CustomError, EmptyCustomError, ErrorResponse, GetSourceFilesResponse,
        VerifyFromEtherscanResponse,
    },
    Error, SourcifyError, VerifyFromEtherscanError,
};
use blockscout_display_bytes::Bytes as DisplayBytes;
use bytes::Bytes;
use reqwest::{Response, StatusCode};
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};
use std::{str::FromStr, time::Duration};
use url::Url;

#[derive(Clone)]
pub struct ClientBuilder {
    base_url: Url,
    total_duration: Duration,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            base_url: Url::from_str("https://sourcify.dev/server/").unwrap(),
            total_duration: Duration::from_secs(180),
        }
    }
}

impl ClientBuilder {
    pub fn try_base_url(mut self, base_url: &str) -> Result<Self, String> {
        let base_url = Url::from_str(base_url).map_err(|err| err.to_string())?;
        self.base_url = base_url;

        Ok(self)
    }

    pub fn total_duration(mut self, total_duration: Duration) -> Self {
        self.total_duration = total_duration;
        self
    }

    pub fn build(self) -> Client {
        let retry_policy =
            ExponentialBackoff::builder().build_with_total_retry_duration(self.total_duration);
        let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Client {
            base_url: self.base_url,
            reqwest_client: client,
        }
    }
}

#[derive(Clone)]
pub struct Client {
    base_url: Url,
    reqwest_client: ClientWithMiddleware,
}

impl Default for Client {
    /// Initializes [`Client`] with base url set to "https://sourcify.dev/server/",
    /// and total duration to 60 seconds.
    fn default() -> Self {
        ClientBuilder::default().build()
    }
}

impl Client {
    pub async fn get_source_files_any(
        &self,
        chain_id: &str,
        contract_address: Bytes,
    ) -> Result<GetSourceFilesResponse, Error<EmptyCustomError>> {
        let contract_address = DisplayBytes::from(contract_address);
        let url =
            self.generate_url(format!("files/any/{}/{}", chain_id, contract_address).as_str());

        let response = self
            .reqwest_client
            .get(url)
            .send()
            .await
            .map_err(|error| match error {
                reqwest_middleware::Error::Middleware(err) => Error::ReqwestMiddleware(err),
                reqwest_middleware::Error::Reqwest(err) => Error::Reqwest(err),
            })?;

        Self::process_sourcify_response(response).await
    }

    pub async fn verify_from_etherscan(
        &self,
        chain_id: &str,
        contract_address: Bytes,
    ) -> Result<VerifyFromEtherscanResponse, Error<VerifyFromEtherscanError>> {
        let contract_address = DisplayBytes::from(contract_address);
        let url = self.generate_url("verify/etherscan");

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            chain_id: &'a str,
            address: String,
        }

        let request = Request {
            chain_id,
            address: contract_address.to_string(),
        };

        let response = self
            .reqwest_client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|error| match error {
                reqwest_middleware::Error::Middleware(err) => Error::ReqwestMiddleware(err),
                reqwest_middleware::Error::Reqwest(err) => Error::Reqwest(err),
            })?;

        Self::process_sourcify_response(response).await
    }
}

impl Client {
    fn generate_url(&self, route: &str) -> Url {
        self.base_url.join(route).unwrap()
    }

    async fn process_sourcify_response<T: for<'de> Deserialize<'de>, E: CustomError>(
        response: Response,
    ) -> Result<T, Error<E>> {
        let error_message = |response: Response| async {
            response
                .json::<ErrorResponse>()
                .await
                .map(|value| value.error)
        };

        match response.status() {
            StatusCode::OK => Ok(response.json::<T>().await?),
            StatusCode::NOT_FOUND => {
                let message = error_message(response).await?;
                if let Some(err) = E::handle_not_found(&message) {
                    Err(Error::Sourcify(SourcifyError::Custom(err)))
                } else {
                    Err(Error::Sourcify(SourcifyError::NotFound(message)))
                }
            }
            StatusCode::BAD_REQUEST => {
                let message = error_message(response).await?;
                if let Some(err) = E::handle_bad_request(&message) {
                    Err(Error::Sourcify(SourcifyError::Custom(err)))
                } else {
                    Err(Error::Sourcify(SourcifyError::BadRequest(message)))
                }
            }
            StatusCode::INTERNAL_SERVER_ERROR => {
                let message = error_message(response).await?;
                if let Some(err) = E::handle_internal_server_error(&message) {
                    Err(Error::Sourcify(SourcifyError::Custom(err)))
                } else {
                    // For now the only way to recognize that the chain is not supported by Sourcify.
                    // Message example: "Chain 134135 is not a Sourcify chain!"
                    if message.contains("is not a Sourcify chain") {
                        Err(Error::Sourcify(SourcifyError::ChainNotSupported(message)))
                    } else {
                        Err(Error::Sourcify(SourcifyError::InternalServerError(message)))
                    }
                }
            }
            status_code => {
                let text = response.text().await?;
                if let Some(err) = E::handle_status_code(status_code, &text) {
                    Err(Error::Sourcify(SourcifyError::Custom(err)))
                } else {
                    Err(Error::Sourcify(SourcifyError::UnexpectedStatusCode {
                        status_code,
                        msg: text,
                    }))
                }
            }
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use serde_json::json;
//
//     fn parse_contract_address(contract_address: &str) -> Bytes {
//         DisplayBytes::from_str(contract_address).unwrap().0
//     }
//
//     mod success {
//         use super::*;
//         use pretty_assertions::assert_eq;
//
//         #[tokio::test]
//         async fn get_source_files_any() {
//             let expected: GetSourceFilesResponse = serde_json::from_value(json!({
//             "status": "full",
//             "files": [
//                 {
//                     "name": "library-map.json",
//                     "path": "/home/data/repository/contracts/full_match/5/0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52/library-map.json",
//                     "content": "{\"__$54103d3e1543ebb87230c9454f838057a5$__\":\"6b88c55cfbd4eda1320f802b724193cab062ccce\"}"
//                 },
//                 {
//                     "name": "metadata.json",
//                     "path": "/home/data/repository/contracts/full_match/5/0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52/metadata.json",
//                     "content": "{\"compiler\":{\"version\":\"0.6.8+commit.0bbfe453\"},\"language\":\"Solidity\",\"output\":{\"abi\":[{\"anonymous\":false,\"inputs\":[],\"name\":\"SourcifySolidity14\",\"type\":\"event\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"input\",\"type\":\"address\"}],\"name\":\"identity\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"\",\"type\":\"address\"}],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}],\"devdoc\":{\"methods\":{}},\"userdoc\":{\"methods\":{}}},\"settings\":{\"compilationTarget\":{\"contracts/project:/ExternalTestMultiple.sol\":\"ExternalTestMultiple\"},\"evmVersion\":\"istanbul\",\"libraries\":{},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"optimizer\":{\"enabled\":true,\"runs\":300},\"remappings\":[]},\"sources\":{\"contracts/project:/ExternalTestMultiple.sol\":{\"keccak256\":\"0xc40380283b7d4a97da5e247fbb7b795f6241cfe3d86e34493d87528dfcb4d56b\",\"license\":\"MIT\",\"urls\":[\"bzz-raw://86ec578963cb912c4b912f066390e564c54ea1bc5fb1a55aa4e4c77bb92b07ba\",\"dweb:/ipfs/QmeqihJa8kUjbNHNCpFRHkq1scCbjjFvaUN2gWEJCNEx1Q\"]},\"contracts/project_/ExternalTestMultiple.sol\":{\"keccak256\":\"0xff9e0ddd21b0579491371fe8d4f7e09254ffc7af9382ba287ef8d2a2fd1ce8e2\",\"license\":\"MIT\",\"urls\":[\"bzz-raw://1f516a34091c829a18a8c5dd13fbd82f44b532e7dea6fed9f60ae731c9042d74\",\"dweb:/ipfs/QmZqm6CLGUKQ3RJCLAZy5CWo2ScLzV2r5JXWNWfBwbGCsK\"]}},\"version\":1}"
//                 },
//                 {
//                     "name": "ExternalTestMultiple.sol",
//                     "path": "/home/data/repository/contracts/full_match/5/0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52/sources/contracts/project_/ExternalTestMultiple.sol",
//                     "content": "//SPDX-License-Identifier: MIT\r\npragma solidity ^0.6.8;\r\n\r\nlibrary ExternalTestLibraryMultiple {\r\n  function pop(address[] storage list) external returns (address out) {\r\n    out = list[list.length - 1];\r\n    list.pop();\r\n  }\r\n}\r\n"
//                 }
//             ]
//         })).unwrap();
//
//             let chain_id = "5";
//             let contract_address =
//                 parse_contract_address("0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52");
//
//             let result = Client::default()
//                 .get_source_files_any(chain_id, contract_address)
//                 .await
//                 .expect("success expected");
//             assert_eq!(expected, result);
//         }
//
//         #[tokio::test]
//         async fn verify_from_etherscan() {
//             let expected: VerifyFromEtherscanResponse = serde_json::from_value(json!({
//             "result": [
//                 {
//                     "address": "0x831b003398106153eD89a758bEC9734667D18AeC",
//                     "chainId": "10",
//                     "status": "partial",
//                     "libraryMap": {
//                         "__$5762d9689e001ee319dd424b89cc702f5c$__": "9224ee604e9b62f8e0a0e5824fee2e0df2ca902f"
//                     },
//                     "immutableReferences": {"2155":[{"length":32,"start":4157},{"length":32,"start":4712}],"2157":[{"length":32,"start":1172},{"length":32,"start":1221},{"length":32,"start":1289},{"length":32,"start":2077},{"length":32,"start":4218},{"length":32,"start":5837}],"2159":[{"length":32,"start":742},{"length":32,"start":4943}],"2161":[{"length":32,"start":402},{"length":32,"start":3247},{"length":32,"start":5564}]}
//                 }
//             ]
//         })).unwrap();
//
//             let chain_id = "10";
//             let contract_address =
//                 parse_contract_address("0x831b003398106153eD89a758bEC9734667D18AeC");
//
//             let result = Client::default()
//                 .verify_from_etherscan(chain_id, contract_address)
//                 .await
//                 .expect("success expected");
//             assert_eq!(expected, result);
//         }
//     }
//
//     mod not_found {
//         use super::*;
//
//         #[tokio::test]
//         async fn get_source_files_any() {
//             let chain_id = "5";
//             let contract_address =
//                 parse_contract_address("0x847F2d0c193E90963aAD7B2791aAE8d7310dFF6A");
//
//             let result = Client::default()
//                 .get_source_files_any(chain_id, contract_address)
//                 .await
//                 .expect_err("error expected");
//             assert!(
//                 matches!(result, Error::Sourcify(SourcifyError::NotFound(_))),
//                 "expected: 'SourcifyError::NotFound', got: {result:?}"
//             );
//         }
//
//         /*
//         * Not implemented, as custom error that 'contract is not verified on Etherscan' is returned instead.
//
//         * async fn verify_from_etherscan() {}
//         */
//     }
//
//     mod bad_request {
//         use super::*;
//
//         #[tokio::test]
//         async fn get_source_files_any_invalid_argument() {
//             let chain_id = "5";
//             let contract_address = parse_contract_address("0xcafecafecafecafe");
//
//             let result = Client::default()
//                 .get_source_files_any(chain_id, contract_address)
//                 .await
//                 .expect_err("error expected");
//             assert!(
//                 matches!(result, Error::Sourcify(SourcifyError::BadRequest(_))),
//                 "expected: 'SourcifyError::BadRequest', got: {result:?}"
//             );
//         }
//
//         #[tokio::test]
//         async fn verify_from_etherscan_invalid_argument() {
//             let chain_id = "5";
//             let contract_address = parse_contract_address("0xcafecafecafecafe");
//
//             let result = Client::default()
//                 .verify_from_etherscan(chain_id, contract_address)
//                 .await
//                 .expect_err("error expected");
//             assert!(
//                 matches!(result, Error::Sourcify(SourcifyError::BadRequest(_))),
//                 "expected: 'SourcifyError::BadRequest', got: {result:?}"
//             );
//         }
//     }
//
//     mod chain_not_supported {
//         use super::*;
//
//         #[tokio::test]
//         async fn get_source_files_any() {
//             let chain_id = "12345";
//             let contract_address =
//                 parse_contract_address("0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52");
//
//             let result = Client::default()
//                 .get_source_files_any(chain_id, contract_address)
//                 .await
//                 .expect_err("error expected");
//             assert!(
//                 matches!(result, Error::Sourcify(SourcifyError::ChainNotSupported(_))),
//                 "expected: 'SourcifyError::BadRequest', got: {result:?}"
//             );
//         }
//
//         /*
//         * Not implemented, as custom error that 'chain is not supported for verification on Etherscan' is returned instead.
//
//         * async fn verify_from_etherscan() {}
//         */
//     }
//
//     mod custom_errors {
//         use super::*;
//
//         #[tokio::test]
//         async fn verify_from_etherscan_chain_is_not_supported() {
//             let chain_id = "2221";
//             let contract_address =
//                 parse_contract_address("0xcb566e3B6934Fa77258d68ea18E931fa75e1aaAa");
//
//             let result = Client::default()
//                 .verify_from_etherscan(chain_id, contract_address)
//                 .await
//                 .expect_err("error expected");
//             assert!(matches!(
//                 result,
//                 Error::Sourcify(SourcifyError::Custom(
//                     VerifyFromEtherscanError::ChainNotSupported(_)
//                 ))
//             ));
//         }
//
//         #[tokio::test]
//         async fn verify_from_etherscan_contract_not_verified() {
//             let chain_id = "5";
//             let contract_address =
//                 parse_contract_address("0x847F2d0c193E90963aAD7B2791aAE8d7310dFF6A");
//
//             let result = Client::default()
//                 .verify_from_etherscan(chain_id, contract_address)
//                 .await
//                 .expect_err("error expected");
//             assert!(matches!(
//                 result,
//                 Error::Sourcify(SourcifyError::Custom(
//                     VerifyFromEtherscanError::ContractNotVerified(_)
//                 ))
//             ));
//         }
//
//         /*
//         * Not implemented to avoid unnecessary burden on the Sourcify server.
//
//         * async fn verify_from_etherscan_too_many_request() {}
//         */
//
//         /*
//         * Not implemented as could not find any contract for which the error is returned.
//         * We need to add the implementation when such contract is found.
//
//         * async fn verify_from_etherscan_api_response_error() {}
//         */
//
//         /*
//         * Not implemented as could not find any contract for which the error is returned.
//         * We need to add the implementation when such contract is found.
//
//         * async fn verify_from_etherscan_cannot_generate_solc_json_input() {}
//         */
//
//         /*
//         * Not implemented as could not find any contract for which the error is returned.
//         * We need to add the implementation when such contract is found.
//
//         * async fn verify_from_etherscan_verified_with_errors() {}
//         */
//     }
// }
