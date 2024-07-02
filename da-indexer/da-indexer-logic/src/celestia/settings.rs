use serde::Deserialize;
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IndexerSettings {
    pub rpc: RpcSettings,
    pub start_height: Option<u64>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RpcSettings {
    pub url: String,
    pub auth_token: Option<String>,
    #[serde(default = "default_max_request_size")]
    pub max_request_size: u32,
    #[serde(default = "default_max_response_size")]
    pub max_response_size: u32,
}

fn default_max_request_size() -> u32 {
    100 * 1024 * 1024 // 100 Mb
}

fn default_max_response_size() -> u32 {
    100 * 1024 * 1024 // 100 Mb
}

impl Default for IndexerSettings {
    fn default() -> Self {
        Self {
            start_height: None,
            rpc: RpcSettings {
                url: "http://localhost:26658".to_string(),
                auth_token: None,
                max_request_size: default_max_request_size(),
                max_response_size: default_max_response_size(),
            },
        }
    }
}
