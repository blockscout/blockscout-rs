use super::{new_client, types::L2BatchMetadata, L2Config};
use anyhow::{anyhow, Result};
use blockscout_display_bytes::Bytes;
use chrono::DateTime;
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct Blob {
    commitment: String,
    height: u64,
    l1_timestamp: String,
    l1_transaction_hash: String,
    namespace: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct L2BatchOptimism {
    batch_data_container: String,
    blobs: Vec<Blob>,
    internal_id: u64,
    l1_timestamp: String,
    l1_tx_hashes: Vec<String>,
    l2_block_start: u64,
    l2_block_end: u64,
    tx_count: u64,
}

pub async fn get_l2_batch(
    config: &L2Config,
    height: u64,
    commitment: &[u8],
) -> Result<Option<L2BatchMetadata>> {
    let commitment = Bytes::from(commitment.to_vec()).to_string();
    let query = format!(
        "{}/api/v2/optimism/batches/da/celestia/{}/{}",
        config.l2_api_url, height, commitment,
    );

    let response = new_client(config)?.get(&query).send().await?;

    if response.status() == StatusCode::NOT_FOUND {
        tracing::debug!(
            height,
            commitment = hex::encode(&commitment),
            "l2 batch metadata not found"
        );
        return Ok(None);
    }
    let mut response: L2BatchOptimism = response.json().await?;

    let l1_tx_hash = response
        .blobs
        .iter()
        .find(|blob| blob.commitment == commitment)
        .ok_or(anyhow!("l1 transaction hash not found"))?
        .l1_transaction_hash
        .clone();

    let related_blobs = response
        .blobs
        .drain(..)
        .filter(|blob| blob.commitment != commitment)
        .map(|blob| super::types::CelestiaBlobId {
            height: blob.height,
            namespace: blob.namespace,
            commitment: blob.commitment,
        })
        .collect();

    Ok(Some(L2BatchMetadata {
        chain_type: super::types::L2Type::Optimism,
        l2_chain_id: config.l2_chain_id,
        l2_batch_id: response.internal_id.to_string(),
        l2_start_block: response.l2_block_start,
        l2_end_block: response.l2_block_end,
        l2_batch_tx_count: response.tx_count as u32,
        l2_blockscout_url: Url::parse(&config.l2_blockscout_url)?
            .join(&format!("batches/{}", response.internal_id))?
            .to_string(),
        l1_tx_hash,
        l1_tx_timestamp: DateTime::parse_from_rfc3339(&response.l1_timestamp)?.timestamp() as u64,
        l1_chain_id: config.l1_chain_id,
        related_blobs,
    }))
}
