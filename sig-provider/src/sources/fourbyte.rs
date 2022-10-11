use crate::{
    proto::blockscout::sig_provider::v1::{
        CreateSignaturesRequest, CreateSignaturesResponse, GetSignaturesRequest,
        GetSignaturesResponse, Signature,
    },
    SignatureProvider,
};

pub struct Provider {
    host: url::Url,
    client: reqwest::Client,
}

impl Provider {
    pub fn new(host: url::Url) -> Provider {
        Provider {
            host,
            client: reqwest::Client::new(),
        }
    }

    async fn fetch(&self, mut path: String) -> Result<GetSignaturesResponse, anyhow::Error> {
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
            signatures.extend(resp.results.into_iter().map(|sig| Signature {
                name: sig.text_signature,
            }));
            if let Some(next) = resp.next {
                path = next;
            } else {
                break;
            }
        }
        Ok(GetSignaturesResponse { signatures })
    }
}

#[async_trait::async_trait]
impl SignatureProvider for Provider {
    async fn create_signatures(
        &self,
        request: CreateSignaturesRequest,
    ) -> Result<CreateSignaturesResponse, anyhow::Error> {
        self.client
            .post(self.host.join("/api/v1/import-solidity/").unwrap())
            .json(&json::CreateRequest {
                contract_abi: request.abi,
            })
            .send()
            .await
            .map(|_| CreateSignaturesResponse {})
            .map_err(anyhow::Error::msg)
    }

    async fn get_function_signatures(
        &self,
        request: GetSignaturesRequest,
    ) -> Result<GetSignaturesResponse, anyhow::Error> {
        self.fetch(format!("/api/v1/signatures/?hex_signature={}", request.hex))
            .await
    }

    async fn get_event_signatures(
        &self,
        request: GetSignaturesRequest,
    ) -> Result<GetSignaturesResponse, anyhow::Error> {
        self.fetch(format!(
            "/api/v1/event-signatures/?hex_signature={}",
            &request.hex
        ))
        .await
    }

    async fn get_error_signatures(
        &self,
        _request: GetSignaturesRequest,
    ) -> Result<GetSignaturesResponse, anyhow::Error> {
        Ok(GetSignaturesResponse {
            signatures: Vec::default(),
        })
    }

    fn host(&self) -> String {
        self.host.to_string()
    }
}

mod json {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize)]
    pub struct CreateRequest {
        pub contract_abi: String,
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
