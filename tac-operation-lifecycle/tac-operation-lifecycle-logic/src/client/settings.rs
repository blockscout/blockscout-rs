use serde::Deserialize;
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct RpcSettings {
    pub url: String,
    #[serde(default = "default_request_per_second")]
    pub request_per_second: u32,
    #[serde(default = "default_num_of_retries")]
    pub num_of_retries: u32,
    #[serde(default = "default_retry_delay_ms")]
    pub retry_delay_ms: u32,
}

fn default_request_per_second() -> u32 {
    100
}

fn default_num_of_retries() -> u32 {
    10
}

fn default_retry_delay_ms() -> u32 {
    1000
}

impl Default for RpcSettings {
    fn default() -> Self {
        Self {
            url: "http://localhost".to_string(),
            request_per_second: default_request_per_second(),
            num_of_retries: default_num_of_retries(),
            retry_delay_ms: default_retry_delay_ms(),
        }
    }
}
