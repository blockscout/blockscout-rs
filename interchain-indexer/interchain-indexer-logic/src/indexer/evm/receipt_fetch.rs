use std::collections::HashMap;

use alloy::{
    network::Ethereum,
    primitives::B256,
    providers::{DynProvider, Provider},
    rpc::types::{Block, Log},
};
use anyhow::{Context, Result};
use futures::{StreamExt, TryStreamExt, stream};

pub(crate) async fn fetch_receipts_for_transactions<I>(
    provider: &DynProvider<Ethereum>,
    transaction_hashes: I,
    concurrency: usize,
) -> Result<HashMap<B256, (Vec<Log>, Block)>>
where
    I: IntoIterator<Item = B256>,
{
    stream::iter(transaction_hashes)
        .map(|hash| async move {
            let receipt = provider
                .get_transaction_receipt(hash)
                .await?
                .context("transaction receipt not found")?;

            let block_number = receipt
                .block_number
                .ok_or(anyhow::anyhow!("missing block number"))?
                .into();

            let block = provider
                .get_block_by_number(block_number)
                .await?
                .context("block not found")?;

            let logs = receipt.inner.logs().to_vec();
            Ok::<(B256, (Vec<Log>, Block)), anyhow::Error>((hash, (logs, block)))
        })
        .buffer_unordered(concurrency.max(1))
        .try_collect()
        .await
}
