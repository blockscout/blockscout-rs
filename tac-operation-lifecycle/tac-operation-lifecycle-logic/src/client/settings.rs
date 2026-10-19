// SPDX-License-Identifier: LicenseRef-Blockscout

use serde::Deserialize;
use serde_with::serde_as;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, serde::Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StageProfilingMode {
    #[default]
    PreferV2,
    V2Only,
    V1Only,
}

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
    #[serde(default)]
    pub stage_profiling_mode: StageProfilingMode,
    #[serde(default = "default_stage_profiling_v2_probe_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub stage_profiling_v2_probe_interval: Duration,
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

fn default_stage_profiling_v2_probe_interval() -> Duration {
    Duration::from_secs(60)
}

impl Default for RpcSettings {
    fn default() -> Self {
        Self {
            url: "http://localhost".to_string(),
            request_per_second: default_request_per_second(),
            num_of_retries: default_num_of_retries(),
            retry_delay_ms: default_retry_delay_ms(),
            stage_profiling_mode: StageProfilingMode::default(),
            stage_profiling_v2_probe_interval: default_stage_profiling_v2_probe_interval(),
        }
    }
}
