use std::time;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum L2Type {
    Optimism,
    Arbitrum,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct L2Config {
    pub l2_chain_type: L2Type,
    pub l2_chain_id: u32,
    pub l2_api_url: String,
    pub l2_blockscout_url: String,
    pub l1_chain_id: Option<u32>,
    #[serde(default = "default_request_timeout")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub request_timeout: time::Duration,
    #[serde(default = "default_request_retries")]
    pub request_retries: u32,
}

fn default_request_timeout() -> time::Duration {
    time::Duration::from_secs(5)
}

fn default_request_retries() -> u32 {
    1
}

pub struct CelestiaBlobId {
    pub namespace: String,
    pub height: u64,
    pub commitment: String,
}

pub struct L2BatchMetadata {
    pub chain_type: L2Type,
    pub l2_chain_id: u32,
    pub l2_batch_id: String,
    pub l2_start_block: u64,
    pub l2_end_block: u64,
    pub l2_batch_tx_count: u32,
    pub l2_blockscout_url: String,
    pub l1_tx_hash: String,
    pub l1_tx_timestamp: u64,
    pub l1_chain_id: Option<u32>,
    pub related_blobs: Vec<CelestiaBlobId>,
}
