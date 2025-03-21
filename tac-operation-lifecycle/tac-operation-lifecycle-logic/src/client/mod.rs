use anyhow::Error;
use reqwest::Response;
use serde::Deserialize;
use settings::RpcSettings;
use tracing::{debug, error};

pub mod settings;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Client {
    rpc: RpcSettings,
}


#[derive(Deserialize, Debug)]
pub struct Operation {
    #[serde(rename = "operation_id")]
    pub id: String,
    pub timestamp: u64,
}

type Operations = Vec<Operation>;

impl Client {
    pub fn new(rpc: RpcSettings) -> Self {
        Self { rpc }
    }

    pub async fn get_operations(&self, start: u64, end: u64) -> Result<Operations, Error> {
        let url = format!("{}/operationIds?from={}&till={}", self.url(), start, end);
        debug!("Fetching operations from URL: {}", url);
        let response: Response = reqwest::get(url).await?;
        let status = response.status();
        debug!("Response status: {}", status);
        
        let text = response.text().await?;
        debug!("Raw response body: {}", text);
        
        if text.is_empty() {
            error!("Received empty response from server");
            return Ok(Vec::new());
        }
        
        #[derive(Deserialize)]
        struct OperationResponse {
            response: Operations,
        }
        
        match serde_json::from_str::<OperationResponse>(&text) {
            Ok(response) => Ok(response.response),
            Err(e) => {
                error!("Failed to parse response: {}", e);
                Err(e.into())
            }
        }
    }

    pub async fn get_operations_v2(&self, start: u64, end: u64) -> Result<Operations, Error> {
        let url = format!("{}/operation-ids?from={}&till={}", self.url(), start, end);
        debug!("Fetching operations from URL: {}", url);
        let response: Response = reqwest::get(url).await?;
        let status = response.status();
        debug!("Response status: {}", status);
        
        let text = response.text().await?;
        debug!("Raw response body: {}", text);
        
        if text.is_empty() {
            error!("Received empty response from server");
            return Ok(Vec::new());
        }
        
        #[derive(Deserialize)]
        struct OperationResponse {
            response: ResponseData,
        }

        #[derive(Deserialize)]
        struct ResponseData {
            total: u32,
            operations: Operations,
        }
        
        match serde_json::from_str::<OperationResponse>(&text) {
            Ok(response) => Ok(response.response.operations),
            Err(e) => {
                error!("Failed to parse response: {}", e);
                Err(e.into())
            }
        }
    }

    pub async fn get_operation_stages_v2(&self, id: &str) -> Result<(), Error> {
        let client = reqwest::Client::new();
        let request_body = serde_json::json!({
            "operationIds": [id]
        });

        match client
            .post(format!("{}/stage-profiling", self.url()))
            .header("accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await {
                Ok(response) => {
                    if response.status().is_success() {
                        let text = response.text().await?;
                        tracing::info!("Raw stage-profiling response body: {}", text);
                        
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!("HTTP error {}: {}", response.status().as_u16(), response.status().as_str()))
                    }
                }
                Err(e) => {
                    Err(e.into())
                }
            }
    }

    fn url(&self) -> &str {
        self.rpc.url.strip_suffix("/").unwrap_or(self.rpc.url.as_str())
    }
}

impl Default for Client {
    fn default() -> Self {
        Self { rpc: RpcSettings::default() }
    }
}
