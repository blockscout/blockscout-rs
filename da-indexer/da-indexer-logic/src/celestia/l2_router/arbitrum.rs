use std::time::Duration;

use super::types::{L2BatchMetadata, L2Config};
use anyhow::Result;
use blockscout_display_bytes::Bytes;
use chrono::DateTime;
use reqwest::{Client, Url};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct CommitmentTransaction {
    hash: String,
    timestamp: String,
}

#[derive(Deserialize, Debug)]
struct L2BatchArbitrum {
    commitment_transaction: CommitmentTransaction,
    end_block: u64,
    number: u64,
    start_block: u64,
    transactions_count: u64,
}

pub async fn get_l2_batch(
    config: &L2Config,
    height: u64,
    commitment: &[u8],
) -> Result<Option<L2BatchMetadata>> {
    let commitment = Bytes::from(commitment.to_vec()).to_string();
    let query = format!(
        "{}/api/v2/arbitrum/batches/da/celestia/{}/{}",
        config.l2_api_url, height, commitment,
    );
    let timeout = Duration::from_secs(5);
    let response: L2BatchArbitrum = Client::new()
        .get(&query)
        .timeout(timeout)
        .send()
        .await?
        .json()
        .await?;

    Ok(Some(L2BatchMetadata {
        chain_type: super::types::L2Type::Arbitrum,
        l2_chain_id: config.l2_chain_id,
        l2_batch_id: response.number.to_string(),
        l2_start_block: response.start_block,
        l2_end_block: response.end_block,
        l2_batch_tx_count: response.transactions_count as u32,
        l2_blockscout_url: Url::parse(&config.l2_blockscout_url)?
            .join(&format!("batches/{}", response.number))?
            .to_string(),
        l1_tx_hash: response.commitment_transaction.hash.clone(),
        l1_tx_timestamp: DateTime::parse_from_rfc3339(&response.commitment_transaction.timestamp)?
            .timestamp() as u64,
        l1_chain_id: config.l1_chain_id,
        related_blobs: vec![], // Arbitrum indexer doesn't support related blobs
    }))
}
