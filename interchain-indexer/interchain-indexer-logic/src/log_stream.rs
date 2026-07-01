// SPDX-License-Identifier: LicenseRef-Blockscout

use alloy::{
    network::Ethereum,
    providers::{DynProvider, Provider},
    rpc::types::{Filter, Log},
};
use anyhow::{Error, Result};
use bon::Builder;
use futures::{StreamExt, stream};
use std::{sync::Arc, time::Duration};

use crate::{
    InterchainDatabase,
    log_stream::log_stream_builder::{IsSet, IsUnset, SetEnableCatchup, SetEnableRealtime, State},
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
    /// Database handle used to persist the `catchup_max_cursor` checkpoint
    /// when the catchup stream finishes scanning down to `genesis_block`.
    /// Required for catchup completion to survive restarts.
    db: Option<Arc<InterchainDatabase>>,
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
        let realtime_cursor = self.realtime_cursor;
        let bridge_id = self.bridge_id;
        let chain_id = self.chain_id;
        let db = self.db.clone();

        tracing::info!(
            bridge_id,
            chain_id,
            genesis_block,
            backward_cursor,
            "Starting catchup stream"
        );

        async_stream::stream! {
            let mut to_block = backward_cursor;
            let mut observed_logs = false;
            while to_block >= genesis_block {
                let from_block = to_block.saturating_sub(batch_span).max(genesis_block);
                tracing::info!(bridge_id, chain_id, from_block, to_block, size =? (to_block - from_block + 1), "scanning CATCHUP  logs");

                match fetch_logs(provider.clone(), &filter, from_block, to_block).await {
                    Ok(logs) => {
                        tracing::debug!(
                            bridge_id,
                            chain_id,
                            count = logs.len(),
                            from_block,
                            to_block,
                            "fetched catchup logs"
                        );
                        if !logs.is_empty() {
                            observed_logs = true;
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

                tokio::time::sleep(poll_interval).await;
            }

            let realtime_cursor_on_insert = safe_catchup_completion_realtime_cursor(
                genesis_block,
                backward_cursor,
                realtime_cursor,
                observed_logs,
            );

            persist_catchup_complete(
                db.as_deref(),
                bridge_id,
                chain_id,
                genesis_block,
                realtime_cursor_on_insert,
            )
            .await;

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

        tracing::info!(
            bridge_id,
            chain_id,
            realtime_cursor,
            "Starting realtime stream"
        );

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

                tracing::info!(bridge_id, chain_id, from_block, to_block, size =? (to_block - from_block + 1), "scanning REALTIME logs");
                match fetch_logs(provider.clone(), &filter, from_block, to_block).await {
                    Ok(logs) => {
                        if !logs.is_empty() {
                            tracing::debug!(
                                bridge_id,
                                chain_id,
                                count = logs.len(),
                                from_block,
                                to_block,
                                batch_size,
                                "fetched realtime logs"
                            );
                            yield logs;
                        }
                        from_block = to_block + 1;
                    }
                    Err(e) => {
                        if is_get_logs_error_silent(&e) {
                            tracing::debug!(
                                err =? e,
                                bridge_id,
                                chain_id,
                                from_block,
                                to_block,
                                "realtime logs are not available at the reported latest block yet, retrying"
                            );
                        } else {
                            tracing::error!(
                                err =? e,
                                bridge_id,
                                chain_id,
                                from_block,
                                to_block,
                                "failed to fetch realtime logs"
                            );
                        }
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

fn is_get_logs_error_silent(err: &Error) -> bool {
    err.chain().any(|cause| {
        cause
            .to_string()
            .contains("from block is greater than latest block")
    })
}

/// Persist that catchup has scanned down to `genesis_block`. Without this,
/// `catchup_max_cursor` would remain at the last observed message and a
/// restart would re-walk the empty range below it on every boot.
async fn persist_catchup_complete(
    db: Option<&InterchainDatabase>,
    bridge_id: Option<i32>,
    chain_id: Option<i64>,
    genesis_block: u64,
    realtime_cursor_on_insert: Option<u64>,
) {
    let (Some(db), Some(bridge_id), Some(chain_id)) = (db, bridge_id, chain_id) else {
        tracing::warn!(
            bridge_id,
            chain_id,
            "skipping catchup checkpoint persistence: db/bridge_id/chain_id missing"
        );
        return;
    };

    if let Err(err) = db
        .mark_catchup_complete(
            bridge_id as u64,
            chain_id as u64,
            genesis_block,
            realtime_cursor_on_insert,
        )
        .await
    {
        tracing::error!(
            err = ?err,
            bridge_id,
            chain_id,
            genesis_block,
            realtime_cursor_on_insert,
            "failed to persist catchup completion checkpoint"
        );
    }
}

fn safe_catchup_completion_realtime_cursor(
    genesis_block: u64,
    catchup_cursor: u64,
    realtime_cursor: u64,
    observed_catchup_logs: bool,
) -> Option<u64> {
    if observed_catchup_logs || realtime_cursor < genesis_block {
        return None;
    }

    // `realtime_cursor` is an inclusive realtime start block, not an
    // already-processed cursor. It is safe to persist on insert only when the
    // completed catchup pass covered every configured block below it and found
    // no logs; otherwise a restart should rescan instead of skipping forward.
    (catchup_cursor.saturating_add(1) >= realtime_cursor).then_some(realtime_cursor)
}

#[cfg(test)]
mod tests {
    use anyhow::Context;

    use super::*;

    #[test]
    fn recognizes_realtime_tip_ahead_error() {
        let err = anyhow::anyhow!(
            "server returned an error response: error code -32602: invalid params, data: \"from block is greater than latest block\""
        );

        assert!(is_get_logs_error_silent(&err));
    }

    #[test]
    fn recognizes_realtime_tip_ahead_error_in_source_chain() {
        let err = Err::<(), _>(anyhow::anyhow!(
            "server returned an error response: error code -32602: invalid params, data: \"from block is greater than latest block\""
        ))
        .context("failed to fetch logs")
        .unwrap_err();

        assert!(is_get_logs_error_silent(&err));
    }

    #[test]
    fn ignores_other_realtime_errors() {
        let err = anyhow::anyhow!(
            "server returned an error response: error code -32005: query returned more than 10000 results"
        );

        assert!(!is_get_logs_error_silent(&err));
    }

    #[test]
    fn safe_catchup_completion_realtime_cursor_allows_empty_contiguous_range() {
        let cursor = safe_catchup_completion_realtime_cursor(10, 99, 100, false);

        assert_eq!(cursor, Some(100));
    }

    #[test]
    fn safe_catchup_completion_realtime_cursor_rejects_observed_logs() {
        let cursor = safe_catchup_completion_realtime_cursor(10, 99, 100, true);

        assert_eq!(cursor, None);
    }

    #[test]
    fn safe_catchup_completion_realtime_cursor_rejects_gap_before_realtime() {
        let cursor = safe_catchup_completion_realtime_cursor(10, 90, 100, false);

        assert_eq!(cursor, None);
    }

    #[test]
    fn safe_catchup_completion_realtime_cursor_rejects_invalid_insert_state() {
        let cursor = safe_catchup_completion_realtime_cursor(100, 99, 50, false);

        assert_eq!(cursor, None);
    }
}
