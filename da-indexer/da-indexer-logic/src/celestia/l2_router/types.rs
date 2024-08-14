use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum L2Type {
    Optimism,
    Arbitrum,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct L2Config {
    pub chain_type: L2Type,
    pub chain_id: u32,
    pub l2_api_url: String,
    pub l2_blockscout_url: String,
}

pub struct CelestiaBlobId {
    pub namespace: String,
    pub height: u64,
    pub commitment: String,
}

pub struct L2BatchMetadata {
    pub chain_type: L2Type,
    pub chain_id: u32,
    pub l2_batch_id: String,
    pub l2_start_block: u64,
    pub l2_end_block: u64,
    pub l2_batch_tx_count: u32,
    pub l2_blockscout_url: String,
    pub l1_tx_hash: String,
    pub l1_tx_timestamp: u64,
    pub related_blobs: Vec<CelestiaBlobId>,
}
