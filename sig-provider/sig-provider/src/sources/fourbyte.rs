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

    async fn fetch(&self, mut path: String) -> Result<Vec<String>, anyhow::Error> {
        let mut signatures = Vec::default();
        loop {
            let resp: json::GetResponse = self
                .client
                .get(self.host.join(&path).unwrap())
                .send()
                .await
                .map_err(anyhow::Error::msg)?
                .json()
                .await
                .map_err(anyhow::Error::msg)?;
            signatures.extend(resp.results.into_iter().map(|sig| sig.text_signature));
            if let Some(next) = resp.next {
                path = next;
            } else {
                break;
            }
        }
        // TODO: sort using "id" field
        Ok(signatures)
    }
}

#[async_trait::async_trait]
impl SignatureSource for Source {
    async fn create_signatures(&self, abi: &str) -> Result<(), anyhow::Error> {
        self.client
            .post(self.host.join("/api/v1/import-solidity/").unwrap())
            .json(&json::CreateRequest { contract_abi: abi })
            .send()
            .await
            .map(|_| ())
            .map_err(anyhow::Error::msg)
    }

    async fn get_function_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error> {
        self.fetch(format!("/api/v1/signatures/?hex_signature={}", hex))
            .await
    }

    async fn get_event_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error> {
        self.fetch(format!("/api/v1/event-signatures/?hex_signature={}", hex))
            .await
    }

    fn source(&self) -> String {
        self.host.to_string()
    }
}

mod json {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize)]
    pub struct CreateRequest<'a> {
        pub contract_abi: &'a str,
    }

    #[derive(Debug, Deserialize)]
    pub struct Signature {
        pub text_signature: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct GetResponse {
        pub next: Option<String>,
        pub results: Vec<Signature>,
    }
}
