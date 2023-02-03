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

    fn hash(hex: &str) -> String {
        if hex.starts_with("0x") {
            hex.to_owned()
        } else {
            "0x".to_owned() + hex
        }
    }

    async fn fetch(&self, path: &str) -> Result<json::GetResponse, anyhow::Error> {
        self.client
            .get(self.host.join(path).unwrap())
            .send()
            .await
            .map_err(anyhow::Error::msg)?
            .json()
            .await
            .map_err(anyhow::Error::msg)
    }

    fn convert(sigs: Option<json::SigMap>, hash: &str) -> Vec<String> {
        // TODO: sort using "filtered" field
        sigs.and_then(|mut sigs| {
            sigs.remove(hash)
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
            .post(self.host.join("/api/v1/import").unwrap())
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
        let hash = Self::hash(hex);
        let resp = self
            .fetch(&format!("/api/v1/signatures?function={hash}&all"))
            .await?;
        let signatures = Self::convert(resp.result.function, &hash);
        Ok(signatures)
    }

    async fn get_event_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error> {
        let hash = Self::hash(hex);
        let resp = self
            .fetch(&format!("/api/v1/signatures?event={hash}&all"))
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

    pub type SigMap = HashMap<String, Vec<Signature>>;

    #[derive(Debug, Deserialize)]
    pub struct SigTypes {
        pub function: Option<SigMap>,
        pub event: Option<SigMap>,
        pub error: Option<SigMap>,
    }

    #[derive(Debug, Deserialize)]
    pub struct GetResponse {
        pub result: SigTypes,
    }
}
