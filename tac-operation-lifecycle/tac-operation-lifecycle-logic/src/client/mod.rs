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
        let url = format!("{}/operationIds?from={}&to={}", self.rpc.url, start, end);
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
}

impl Default for Client {
    fn default() -> Self {
        Self { rpc: RpcSettings::default() }
    }
}
