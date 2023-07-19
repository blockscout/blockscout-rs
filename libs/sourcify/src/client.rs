use crate::{
    types::{Error as SourcifyError, GetSourceFilesResponse},
    Error,
};
use blockscout_display_bytes::Bytes as DisplayBytes;
use bytes::Bytes;
use reqwest::{Response, StatusCode};
use serde::Deserialize;
use std::{str::FromStr, time::Duration};
use url::Url;

#[derive(Clone)]
pub struct Client {
    base_url: Url,
    reqwest_client: reqwest::Client,
}

impl Default for Client {
    fn default() -> Self {
        let base_url = "https://sourcify.dev/server/";
        Client::with_base_url(base_url).unwrap()
    }
}

impl Client {
    pub fn with_base_url(base_url: &str) -> Result<Self, Error> {
        let base_url = Url::from_str(base_url).map_err(|err| Error::InvalidArgument {
            arg: "base_url".to_string(),
            error: err.to_string(),
        })?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self {
            base_url,
            reqwest_client: client,
        })
    }

    pub async fn get_source_files_any(
        &self,
        chain_id: &str,
        contract_address: Bytes,
    ) -> Result<GetSourceFilesResponse, Error> {
        let contract_address = DisplayBytes::from(contract_address);
        let url = self
            .base_url
            .join(format!("files/any/{}/{}", chain_id, contract_address).as_str())
            .unwrap();

        let response = self.reqwest_client.get(url).send().await?;

        Self::process_sourcify_response(response).await
    }
}

impl Client {
    async fn process_sourcify_response<T: for<'de> Deserialize<'de>>(
        response: Response,
    ) -> Result<T, Error> {
        let error_message = |response: Response| async {
            response
                .json::<SourcifyError>()
                .await
                .map(|value| value.error)
        };

        match response.status() {
            StatusCode::OK => Ok(response.json::<T>().await?),
            StatusCode::NOT_FOUND => Err(Error::SourcifyNotFound(error_message(response).await?)),
            StatusCode::BAD_REQUEST => {
                Err(Error::SourcifyBadRequest(error_message(response).await?))
            }
            StatusCode::INTERNAL_SERVER_ERROR => Err(Error::SourcifyInternalServerError(
                error_message(response).await?,
            )),
            StatusCode::TOO_MANY_REQUESTS => Err(Error::SourcifyTooManyRequests(
                error_message(response).await?,
            )),
            _ => Err(Error::SourcifyUnexpectedStatusCode {
                status_code: response.status(),
                msg: response.text().await?,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazy_static::lazy_static;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    lazy_static! {
        static ref CLIENT: Client = Client::default();
    }

    fn parse_contract_address(contract_address: &str) -> Bytes {
        DisplayBytes::from_str(contract_address).unwrap().0
    }

    #[tokio::test]
    async fn get_source_files_any_success() {
        let expected: GetSourceFilesResponse = serde_json::from_value(json!({
            "status": "full",
            "files": [
                {
                    "name": "library-map.json",
                    "path": "/home/data/repository/contracts/full_match/5/0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52/library-map.json",
                    "content": "{\"__$54103d3e1543ebb87230c9454f838057a5$__\":\"6b88c55cfbd4eda1320f802b724193cab062ccce\"}"
                },
                {
                    "name": "metadata.json",
                    "path": "/home/data/repository/contracts/full_match/5/0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52/metadata.json",
                    "content": "{\"compiler\":{\"version\":\"0.6.8+commit.0bbfe453\"},\"language\":\"Solidity\",\"output\":{\"abi\":[{\"anonymous\":false,\"inputs\":[],\"name\":\"SourcifySolidity14\",\"type\":\"event\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"input\",\"type\":\"address\"}],\"name\":\"identity\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"\",\"type\":\"address\"}],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}],\"devdoc\":{\"methods\":{}},\"userdoc\":{\"methods\":{}}},\"settings\":{\"compilationTarget\":{\"contracts/project:/ExternalTestMultiple.sol\":\"ExternalTestMultiple\"},\"evmVersion\":\"istanbul\",\"libraries\":{},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"optimizer\":{\"enabled\":true,\"runs\":300},\"remappings\":[]},\"sources\":{\"contracts/project:/ExternalTestMultiple.sol\":{\"keccak256\":\"0xc40380283b7d4a97da5e247fbb7b795f6241cfe3d86e34493d87528dfcb4d56b\",\"license\":\"MIT\",\"urls\":[\"bzz-raw://86ec578963cb912c4b912f066390e564c54ea1bc5fb1a55aa4e4c77bb92b07ba\",\"dweb:/ipfs/QmeqihJa8kUjbNHNCpFRHkq1scCbjjFvaUN2gWEJCNEx1Q\"]},\"contracts/project_/ExternalTestMultiple.sol\":{\"keccak256\":\"0xff9e0ddd21b0579491371fe8d4f7e09254ffc7af9382ba287ef8d2a2fd1ce8e2\",\"license\":\"MIT\",\"urls\":[\"bzz-raw://1f516a34091c829a18a8c5dd13fbd82f44b532e7dea6fed9f60ae731c9042d74\",\"dweb:/ipfs/QmZqm6CLGUKQ3RJCLAZy5CWo2ScLzV2r5JXWNWfBwbGCsK\"]}},\"version\":1}"
                },
                {
                    "name": "ExternalTestMultiple.sol",
                    "path": "/home/data/repository/contracts/full_match/5/0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52/sources/contracts/project_/ExternalTestMultiple.sol",
                    "content": "//SPDX-License-Identifier: MIT\r\npragma solidity ^0.6.8;\r\n\r\nlibrary ExternalTestLibraryMultiple {\r\n  function pop(address[] storage list) external returns (address out) {\r\n    out = list[list.length - 1];\r\n    list.pop();\r\n  }\r\n}\r\n"
                }
            ]
        })).unwrap();

        let chain_id = "5";
        let contract_address = parse_contract_address("0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52");

        let result = CLIENT
            .get_source_files_any(chain_id, contract_address)
            .await
            .expect("success expected");
        assert_eq!(expected, result);
    }

    #[tokio::test]
    async fn get_source_files_any_not_found() {
        let chain_id = "5";
        let contract_address = parse_contract_address("0x8A81C1619f38a5bb29cfaf20dB24B23F42A42dCb");

        let result = CLIENT
            .get_source_files_any(chain_id, contract_address)
            .await
            .expect_err("error expected");
        assert!(
            matches!(result, Error::SourcifyNotFound(_)),
            "expected: 'SourcifyNotFound', got: {result:?}"
        );
    }

    #[tokio::test]
    async fn get_source_files_any_invalid_argument() {
        let chain_id = "5";
        let contract_address = parse_contract_address("0xcafecafecafecafe");

        let result = CLIENT
            .get_source_files_any(chain_id, contract_address)
            .await
            .expect_err("error expected");
        assert!(
            matches!(result, Error::SourcifyBadRequest(_)),
            "expected: 'SourcifyBadRequest', got: {result:?}"
        );
    }
}
