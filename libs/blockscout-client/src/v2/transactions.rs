use super::types;
use crate::{Client, Result};

pub async fn get(
    client: &Client,
    transaction_hash: ethers_core::types::TxHash,
) -> Result<types::Transaction> {
    let path = format!("/api/v2/transactions/0x{}", hex::encode(transaction_hash));
    client.get_request(client.build_url(&path)).await
}
