use alloy::{
    providers::Provider,
    rpc::types::{Filter, Log},
};
use anyhow::Result;
use futures::{StreamExt, stream};
use std::{sync::Arc, time::Duration};

pub struct LogStreamBuilder {
    provider: Arc<dyn Provider + Send + Sync>,
    filter: Filter,
    genesis_block: u64,
    forward_cursor: u64,
    backward_cursor: u64,
    poll_interval: Duration,
    batch_size: u64,
    stream: stream::BoxStream<'static, Vec<Log>>,
}

impl LogStreamBuilder {
    pub fn new(provider: Arc<dyn Provider + Send + Sync>) -> Self {
        Self {
            provider,
            filter: Filter::default(),
            genesis_block: 0,
            forward_cursor: 0,
            backward_cursor: 0,
            stream: stream::empty::<Vec<Log>>().boxed(),
            poll_interval: Duration::from_secs(10),
            batch_size: 100,
        }
    }

    pub fn filter(mut self, filter: Filter) -> Self {
        self.filter = filter;
        self
    }

    pub fn genesis_block(mut self, genesis_block: u64) -> Self {
        self.genesis_block = genesis_block;
        self
    }

    pub fn poll_interval(mut self, poll_interval: Duration) -> Self {
        self.poll_interval = poll_interval;
        self
    }

    pub fn batch_size(mut self, batch_size: u64) -> Self {
        self.batch_size = batch_size;
        self
    }

    pub fn forward_cursor(mut self, forward_cursor: u64) -> Self {
        self.forward_cursor = forward_cursor;
        self
    }

    pub fn backward_cursor(mut self, backward_cursor: u64) -> Self {
        self.backward_cursor = backward_cursor;
        self
    }

    pub fn catchup(mut self) -> Self {
        let provider = Arc::clone(&self.provider);
        let filter = self.filter.clone();
        let poll_interval = self.poll_interval;
        let batch_size = self.batch_size;
        let genesis_block = self.genesis_block;
        let backward_cursor = self.backward_cursor;

        let stream = async_stream::stream! {
            let mut to_block = backward_cursor;
            while to_block >= genesis_block {
                let from_block = to_block.saturating_sub(batch_size).max(genesis_block);
                tracing::debug!(from_block, to_block, "catchup logs batch");

                match fetch_logs(&provider, &filter, from_block, to_block).await {
                    Ok(logs) => {
                        tracing::info!(
                            count = logs.len(),
                            from_block,
                            to_block,
                            "fetched catchup logs"
                        );
                        if !logs.is_empty() {
                            yield logs;
                        }
                        to_block = from_block.saturating_sub(1);
                    }
                    Err(e) => {
                        tracing::error!(
                            err =? e,
                            from_block,
                            to_block,
                            "failed to fetch catchup logs batch, retrying"
                        );
                        tokio::time::sleep(poll_interval).await;
                        continue;
                    }
                }
            }

            tracing::info!(genesis_block, "catchup complete, reached genesis block");
        };

        self.stream = stream::select(self.stream, stream).boxed();
        self
    }

    pub fn realtime(mut self) -> Self {
        let provider = Arc::clone(&self.provider);
        let filter = self.filter.clone();
        let poll_interval = self.poll_interval;
        let batch_size = self.batch_size;
        let forward_cursor = self.forward_cursor;

        let stream = async_stream::stream! {
            let mut from_block = forward_cursor;
            loop {
                let to_block = provider
                    .get_block_number()
                    .await
                    .inspect_err(|e| tracing::error!(err =? e, "failed to get latest block number"))
                    .ok()
                    .filter(|latest| from_block <= *latest)
                    .map(|latest| (from_block + batch_size).min(latest));

                let Some(to_block) = to_block else {
                    tracing::debug!(from_block, "waiting for new blocks");
                    tokio::time::sleep(poll_interval).await;
                    continue;
                };

                match fetch_logs(&provider, &filter, from_block, to_block).await {
                    Ok(logs) => {
                        if !logs.is_empty() {
                            tracing::info!(
                                count = logs.len(),
                                from_block,
                                to_block,
                                "found realtime logs"
                            );
                            yield logs;
                        }
                        from_block = to_block + 1;
                    }
                    Err(e) => {
                        tracing::error!(
                            err =? e,
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
        };

        self.stream = stream::select(self.stream, stream).boxed();
        self
    }

    pub fn into_stream(self) -> stream::BoxStream<'static, Vec<Log>> {
        let ordered_stream = self.stream.map(|mut logs| {
            logs.sort_by_key(|log| (log.block_number, log.log_index));
            logs
        });

        Box::pin(ordered_stream)
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
    provider: &Arc<dyn Provider + Send + Sync>,
    filter: &Filter,
    from_block: u64,
    to_block: u64,
) -> Result<Vec<Log>> {
    let filter = filter.clone().from_block(from_block).to_block(to_block);
    let logs = provider.get_logs(&filter).await?;
    Ok(logs)
}
