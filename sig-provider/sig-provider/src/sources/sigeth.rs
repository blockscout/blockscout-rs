// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::SignatureSource;
use reqwest_middleware::ClientWithMiddleware;

pub struct Source {
    host: url::Url,
    client: ClientWithMiddleware,
}

impl Source {
    pub fn new(host: url::Url) -> Source {
        Source {
            host,
            client: super::new_client(),
        }
    }

    async fn fetch(&self, path: &str) -> Result<json::GetResponse, anyhow::Error> {
        let response = self
            .client
            .get(self.host.join(path).unwrap())
            .send()
            .await
            .map_err(anyhow::Error::msg)?;
        match response.status() {
            reqwest::StatusCode::OK => Ok(response.json().await?),
            status => Err(anyhow::anyhow!(
                "invalid status code got as a result: {}",
                status
            )),
        }
    }

    fn convert(sigs: Option<json::SigMap>, hash: &str) -> Vec<String> {
        // TODO: sort using "filtered" field
        sigs.and_then(|mut sigs| {
            sigs.remove(hash)
                .flatten()
                .map(|sigs| sigs.into_iter().map(|sig| sig.name).collect())
        })
        .unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl SignatureSource for Source {
    async fn create_signatures(&self, abi: &str) -> Result<(), anyhow::Error> {
        let abi = serde_json::from_str(abi).map_err(anyhow::Error::msg)?;
        self.client
            .post(self.host.join("/signature-database/v1/import").unwrap())
            .json(&json::CreateRequest {
                kind: "abi",
                data: vec![abi],
            })
            .send()
            .await
            .map(|_| ())
            .map_err(anyhow::Error::msg)
    }

    async fn get_function_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error> {
        let hash = super::hash(hex);
        let resp = self
            .fetch(&format!(
                "/signature-database/v1/lookup?function={hash}&filter=false"
            ))
            .await?;
        let signatures = Self::convert(resp.result.function, &hash);
        Ok(signatures)
    }

    async fn get_event_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error> {
        let hash = super::hash(hex);
        let resp = self
            .fetch(&format!(
                "/signature-database/v1/lookup?event={hash}&filter=false"
            ))
            .await?;
        let signatures = Self::convert(resp.result.event, &hash);
        Ok(signatures)
    }

    fn source(&self) -> String {
        self.host.to_string()
    }
}

mod json {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize)]
    pub struct CreateRequest {
        #[serde(rename = "type")]
        pub kind: &'static str,
        pub data: Vec<serde_json::Value>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Signature {
        pub name: String,
    }

    // The upstream API returns `null` (rather than an empty list) for a hash
    // it has no signatures for, so the inner value must be optional.
    pub type SigMap = HashMap<String, Option<Vec<Signature>>>;

    #[derive(Debug, Deserialize)]
    pub struct SigTypes {
        pub function: Option<SigMap>,
        pub event: Option<SigMap>,
        pub _error: Option<SigMap>,
    }

    #[derive(Debug, Deserialize)]
    pub struct GetResponse {
        pub result: SigTypes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::MockServer;
    use std::str::FromStr;

    const ABI: &str = r#"[{"constant":false,"inputs":[],"name":"f","outputs":[],"type":"function"},{"inputs":[],"type":"constructor"},{"anonymous":false,"inputs":[{"name":"","type":"string","indexed":true}],"name":"E","type":"event"}]"#;
    const FUNCTION_HEX: &str = "0x26121ff0";
    const EVENT_HEX: &str = "0x3e9992c940c54ea252d3a34557cc3d3014281525c43d694f89d5f3dfd820b07d";

    #[rstest::fixture]
    fn server() -> MockServer {
        MockServer::start()
    }

    fn source(server: &MockServer) -> Source {
        let host = url::Url::from_str(&server.base_url()).expect("mock server url is not an url");
        Source::new(host)
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn create(server: MockServer) {
        let abi: serde_json::Value = serde_json::from_str(ABI).unwrap();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/signature-database/v1/import")
                .json_body(serde_json::json!({"type": "abi", "data": [abi]}));
            then.status(200)
                .header("Content-type", "application/json")
                .json_body(serde_json::json!({"ok":true,"result":{"function":{"imported":{},"duplicated":{"f()":FUNCTION_HEX},"invalid":null},"event":{"imported":{},"duplicated":{"E(string)":EVENT_HEX},"invalid":null}}}));
        });

        source(&server)
            .create_signatures(ABI)
            .await
            .expect("error while submitting a new signature");

        mock.assert();
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn get_function_signatures(server: MockServer) {
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/signature-database/v1/lookup")
                .query_param("function", FUNCTION_HEX)
                .query_param("filter", "false");
            then.status(200)
                .header("Content-type", "application/json")
                .json_body(serde_json::json!({"ok":true,"result":{"function":{FUNCTION_HEX:[{"name":"f()","filtered":false,"hasVerifiedContract":true}]},"event":{}}}));
        });

        let result = source(&server)
            .get_function_signatures(FUNCTION_HEX)
            .await
            .expect("error while getting function signature");

        mock.assert();
        assert_eq!(vec!["f()".to_string()], result);
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn get_event_signatures(server: MockServer) {
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/signature-database/v1/lookup")
                .query_param("event", EVENT_HEX)
                .query_param("filter", "false");
            then.status(200)
                .header("Content-type", "application/json")
                .json_body(serde_json::json!({"ok":true,"result":{"function":{},"event":{EVENT_HEX:[{"name":"E(string)","filtered":false,"hasVerifiedContract":true}]}}}));
        });

        let result = source(&server)
            .get_event_signatures(EVENT_HEX)
            .await
            .expect("error while getting event signature");

        mock.assert();
        assert_eq!(vec!["E(string)".to_string()], result);
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn get_function_signatures_without_match(server: MockServer) {
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/signature-database/v1/lookup")
                .query_param("function", FUNCTION_HEX)
                .query_param("filter", "false");
            then.status(200)
                .header("Content-type", "application/json")
                .json_body(serde_json::json!({"ok":true,"result":{"function":{FUNCTION_HEX:null},"event":{}}}));
        });

        let result = source(&server)
            .get_function_signatures(FUNCTION_HEX)
            .await
            .expect("error while getting function signature");

        mock.assert();
        assert!(result.is_empty(), "expected no signatures, got {result:?}");
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn get_event_signatures_without_match(server: MockServer) {
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/signature-database/v1/lookup")
                .query_param("event", EVENT_HEX)
                .query_param("filter", "false");
            then.status(200)
                .header("Content-type", "application/json")
                .json_body(serde_json::json!({"ok":true,"result":{"function":{},"event":{EVENT_HEX:null}}}));
        });

        let result = source(&server)
            .get_event_signatures(EVENT_HEX)
            .await
            .expect("error while getting event signature");

        mock.assert();
        assert!(result.is_empty(), "expected no signatures, got {result:?}");
    }
}
