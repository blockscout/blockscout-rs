use serde::Deserialize;
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct RpcSettings {
    pub url: String,
    pub auth_token: Option<String>,
    #[serde(default = "default_max_request_size")]
    pub max_request_size: u32,
    #[serde(default = "default_max_response_size")]
    pub max_response_size: u32,
    #[serde(default = "default_request_per_second")]
    pub request_per_second: u32,
}

fn default_max_request_size() -> u32 {
    100 * 1024 * 1024 // 100 Mb
}

fn default_max_response_size() -> u32 {
    100 * 1024 * 1024 // 100 Mb
}

fn default_request_per_second() -> u32 {
    100
}

impl Default for RpcSettings {
    fn default() -> Self {
        Self {
            url: "http://localhost".to_string(),
            auth_token: None,
            max_request_size: default_max_request_size(),
            max_response_size: default_max_response_size(),
            request_per_second: default_request_per_second(),
        }
    }
}
