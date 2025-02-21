use anyhow::Error;
use reqwest::Response;
use serde::Deserialize;
use settings::RpcSettings;

pub mod settings;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Client {
    rpc: RpcSettings,
}


#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub operation_id: String,
    pub timestamp: u64,
}

type Operations = Vec<Operation>;

impl Client {
    pub fn new(rpc: RpcSettings) -> Self {
        Self { rpc }
    }

    pub async fn get_operations(&self, start: u64, end: u64) -> Result<Operations, Error> {
        let url = format!("{}/operations?from={}&to={}", self.rpc.url, start, end);
        let response:Response = reqwest::get(url).await?;
        let operations: Operations = response.json().await?;
        Ok(operations)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self { rpc: RpcSettings::default() }
    }
}
