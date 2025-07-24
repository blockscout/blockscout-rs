use serde::Deserialize;
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IndexerSettings {
    pub disperser_url: String,
    #[serde(default = "disperser_max_decoding_message_size")]
    pub disperser_max_decoding_message_size: usize,
    pub address: String,
    pub rpc: RpcSettings,
    pub start_block: Option<u64>,
    pub creation_block: u64,
    pub save_batch_size: u64,
    pub pruning_block_threshold: u64,
}

/// 32 Mb
fn disperser_max_decoding_message_size() -> usize {
    32 * 1024 * 1024
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RpcSettings {
    pub url: String,
    pub batch_size: u64,
}

impl Default for IndexerSettings {
    fn default() -> Self {
        Self {
            disperser_url: "https://disperser-holesky.eigenda.xyz:443".to_string(),
            disperser_max_decoding_message_size: disperser_max_decoding_message_size(),
            address: "0xD4A7E1Bd8015057293f0D0A557088c286942e84b".to_string(),
            rpc: RpcSettings {
                url: "https://holesky.drpc.org".to_string(),
                batch_size: 10000,
            },
            start_block: None,
            creation_block: 1168412,
            save_batch_size: 20,
            pruning_block_threshold: 1000,
        }
    }
}
