use std::time::Duration;

use alloy::{
    network::Ethereum,
    providers::{DynProvider, Provider},
    rpc::types::{Filter, Log},
};
use anyhow::{Context, Result};
use futures::{StreamExt, stream::BoxStream};

use crate::{InterchainDatabase, log_stream::LogStream};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn build_log_stream_for_chain(
    provider: DynProvider<Ethereum>,
    chain_id: i64,
    bridge_id: i32,
    filter: Filter,
    start_block: u64,
    db: &InterchainDatabase,
    poll_interval: Duration,
    batch_size: u64,
) -> Result<BoxStream<'static, (i64, DynProvider<Ethereum>, Vec<Log>)>> {
    let checkpoint = db.get_checkpoint(bridge_id as u64, chain_id as u64).await?;

    let (realtime_cursor, catchup_cursor) = if let Some(cp) = checkpoint {
        let realtime_cursor = cp.validated_realtime_cursor();
        let catchup_cursor = cp.validated_catchup_cursor();

        tracing::info!(
            bridge_id,
            chain_id,
            realtime_cursor,
            catchup_cursor,
            "restored EVM indexer checkpoint"
        );

        (realtime_cursor, catchup_cursor)
    } else {
        let latest_block = provider
            .get_block_number()
            .await
            .with_context(|| format!("failed to fetch latest block for chain {chain_id}"))?;
        (latest_block, latest_block.saturating_sub(1))
    };

    tracing::info!(bridge_id, chain_id, "configured EVM log stream");

    let stream_provider = provider.clone();
    Ok(LogStream::builder(provider)
        .filter(filter)
        .poll_interval(poll_interval)
        .batch_size(batch_size)
        .genesis_block(start_block)
        .realtime_cursor(realtime_cursor)
        .catchup_cursor(catchup_cursor)
        .bridge_id(bridge_id)
        .chain_id(chain_id)
        .catchup()
        .realtime()
        .build()?
        .map(move |logs| (chain_id, stream_provider.clone(), logs))
        .boxed())
}
