use alloy::{
    network::Ethereum,
    providers::{DynProvider, Provider},
    rpc::types::{Filter, Log},
};
use anyhow::Result;
use bon::Builder;
use futures::{StreamExt, stream};
use std::time::Duration;

use crate::log_stream::log_stream_builder::{
    IsSet, IsUnset, SetEnableCatchup, SetEnableRealtime, State,
};

#[derive(Builder)]
#[builder(finish_fn(name = build_config))]
pub struct LogStream {
    #[builder(start_fn)]
    provider: DynProvider<Ethereum>,
    #[builder(default = Filter::default())]
    filter: Filter,
    #[builder(default = 0)]
    genesis_block: u64,
    #[builder(default = 0)]
    realtime_cursor: u64,
    #[builder(default = 0)]
    catchup_cursor: u64,
    #[builder(default = Duration::from_secs(10))]
    poll_interval: Duration,
    #[builder(default = 100)]
    batch_size: u64,
    bridge_id: Option<i32>,
    chain_id: Option<i64>,
    #[builder(setters(vis = ""))]
    enable_catchup: bool,
    #[builder(setters(vis = ""))]
    enable_realtime: bool,
}

impl<S: State> LogStreamBuilder<S> {
    /// Enable realtime mode, which will continuously poll for new logs starting
    /// from `realtime_cursor` until the stream is dropped.
    pub fn realtime(self) -> LogStreamBuilder<SetEnableRealtime<S>>
    where
        S::EnableRealtime: IsUnset,
    {
        self.enable_realtime(true)
    }

    /// Enable catchup mode, which will fetch historical logs starting from
    /// `catchup_cursor` down to `genesis_block`.
    pub fn catchup(self) -> LogStreamBuilder<SetEnableCatchup<S>>
    where
        S::EnableCatchup: IsUnset,
    {
        self.enable_catchup(true)
    }

    /// Finish the builder and immediately produce the merged log stream.
    pub fn build(self) -> anyhow::Result<stream::BoxStream<'static, Vec<Log>>>
    where
        S::EnableCatchup: IsSet,
        S::EnableRealtime: IsSet,
    {
        self.build_config().into_stream()
    }
}

impl LogStream {
    fn build_catchup_stream(&self) -> stream::BoxStream<'static, Vec<Log>> {
        let provider = self.provider.clone();
        let filter = self.filter.clone();
        let poll_interval = self.poll_interval;
        let batch_size = self.batch_size;
        let batch_span = batch_size.saturating_sub(1);
        let genesis_block = self.genesis_block;
        let backward_cursor = self.catchup_cursor;
        let bridge_id = self.bridge_id;
        let chain_id = self.chain_id;

        async_stream::stream! {
            let mut to_block = backward_cursor;
            while to_block >= genesis_block {
                let from_block = to_block.saturating_sub(batch_span).max(genesis_block);
                tracing::debug!(bridge_id, chain_id, from_block, to_block, batch_size, "catchup logs batch");

                match fetch_logs(provider.clone(), &filter, from_block, to_block).await {
                    Ok(logs) => {
                        tracing::info!(
                            bridge_id,
                            chain_id,
                            count = logs.len(),
                            from_block,
                            to_block,
                            "fetched catchup logs"
                        );
                        if !logs.is_empty() {
                            yield logs;
                        }
                        if from_block == genesis_block {
                            break;
                        }
                        to_block = from_block.saturating_sub(1);
                    }
                    Err(e) => {
                        tracing::error!(
                            err =? e,
                            bridge_id,
                            chain_id,
                            from_block,
                            to_block,
                            "failed to fetch catchup logs batch, retrying"
                        );
                        tokio::time::sleep(poll_interval).await;
                        continue;
                    }
                }
            }

            tracing::info!(bridge_id, chain_id, genesis_block, "catchup complete, reached genesis block");
        }
        .map(|mut logs| {
            logs.sort_by_key(|log| (log.block_number, log.log_index));
            logs.reverse();
            logs
        })
        .boxed()
    }

    fn build_realtime_stream(&self) -> stream::BoxStream<'static, Vec<Log>> {
        let provider = self.provider.clone();
        let filter = self.filter.clone();
        let poll_interval = self.poll_interval;
        let batch_size = self.batch_size;
        let batch_span = batch_size.saturating_sub(1);
        let realtime_cursor = self.realtime_cursor;
        let bridge_id = self.bridge_id;
        let chain_id = self.chain_id;

        async_stream::stream! {
            let mut from_block = realtime_cursor;
            loop {
                let to_block = provider
                    .get_block_number()
                    .await
                    .inspect_err(|e| tracing::error!(err =? e, bridge_id, chain_id, "failed to get latest block number"))
                    .ok()
                    .filter(|latest| from_block <= *latest)
                    .map(|latest| from_block.saturating_add(batch_span).min(latest));

                let Some(to_block) = to_block else {
                    tracing::debug!(bridge_id, chain_id, from_block, "waiting for new blocks");
                    tokio::time::sleep(poll_interval).await;
                    continue;
                };

                match fetch_logs(provider.clone(), &filter, from_block, to_block).await {
                    Ok(logs) => {
                        if !logs.is_empty() {
                            tracing::info!(
                                bridge_id,
                                chain_id,
                                count = logs.len(),
                                from_block,
                                to_block,
                                batch_size,
                                "found realtime logs"
                            );
                            yield logs;
                        }
                        from_block = to_block + 1;
                    }
                    Err(e) => {
                        tracing::error!(
                            err =? e,
                            bridge_id,
                            chain_id,
                            from_block,
                            to_block,
                            "failed to fetch realtime logs"
                        );
                        tokio::time::sleep(poll_interval).await;
                        continue;
                    }
                }

                tokio::time::sleep(poll_interval).await;
            }
        }
        .map(|mut logs| {
            logs.sort_by_key(|log| (log.block_number, log.log_index));
            logs
        })
        .boxed()
    }

    pub fn into_stream(self) -> Result<stream::BoxStream<'static, Vec<Log>>> {
        if self.realtime_cursor < self.catchup_cursor {
            Err(anyhow::anyhow!(
                "realtime_cursor ({}) must be >= catchup_cursor ({})",
                self.realtime_cursor,
                self.catchup_cursor
            ))?;
        };

        let mut combined = stream::empty().boxed();

        if self.enable_catchup {
            combined = stream::select(combined, self.build_catchup_stream()).boxed();
        }

        if self.enable_realtime {
            combined = stream::select(combined, self.build_realtime_stream()).boxed();
        }

        Ok(Box::pin(combined))
    }
}

/// TODO: This fetcher should also be able to continue it's operation if it hits
/// the rpc error (for instance if some eth_getLogs call wasn't successful), it
/// should also be able to dynamically configure block range that is included
/// into this eth_getLogs. for instance, if it fetches block range [0; 100] and
/// block #14 is bad, it should still be able to fetch all blocks that are okay,
/// so [0;13] and [14; 100] should be fetched though and #14 should be marked as
/// a bad block.
async fn fetch_logs(
    provider: DynProvider<Ethereum>,
    filter: &Filter,
    from_block: u64,
    to_block: u64,
) -> Result<Vec<Log>> {
    let filter = filter.clone().from_block(from_block).to_block(to_block);
    let logs = provider.get_logs(&filter).await?;
    Ok(logs)
}
