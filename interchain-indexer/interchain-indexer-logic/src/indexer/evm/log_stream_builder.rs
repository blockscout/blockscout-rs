use std::{sync::Arc, time::Duration};

use alloy::{network::Ethereum, providers::DynProvider, rpc::types::Filter};
use anyhow::{Context, Result, bail};
use futures::{StreamExt, stream::BoxStream};

use crate::{
    InterchainDatabase,
    log_stream::{HeadBlockTag, LogBatch, LogStream, resolve_head_block},
};

/// A stream that reads to `latest`. See
/// [`build_log_stream_for_chain_with_head`] for a confirmed head.
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
) -> Result<BoxStream<'static, (i64, LogBatch)>> {
    build_log_stream_for_chain_with_head(
        provider,
        chain_id,
        bridge_id,
        filter,
        start_block,
        db,
        poll_interval,
        batch_size,
        HeadBlockTag::Latest,
    )
    .await
}

/// `head_tag` bounds the realtime head **and** seeds the cursors of a pair
/// that has no checkpoint yet, so neither realtime nor catch-up ever reads a
/// block above the tagged head.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn build_log_stream_for_chain_with_head(
    provider: DynProvider<Ethereum>,
    chain_id: i64,
    bridge_id: i32,
    filter: Filter,
    start_block: u64,
    db: &InterchainDatabase,
    poll_interval: Duration,
    batch_size: u64,
    head_tag: HeadBlockTag,
) -> Result<BoxStream<'static, (i64, LogBatch)>> {
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
        let head_block = resolve_head_block(&provider, head_tag)
            .await
            .with_context(|| {
                format!(
                    "failed to fetch {} block for chain {chain_id}",
                    head_tag.as_str()
                )
            })?;
        let Some(head_block) = head_block else {
            bail!("chain {chain_id} has no {} block yet", head_tag.as_str());
        };
        (head_block, head_block.saturating_sub(1))
    };

    // Seed / heal the durable scan floor for this pair. A failed write is a
    // `warn`, never a startup blocker: the read-side guard in
    // `CatchupProgress::compute` makes the stored floor cosmetic for the
    // correctness of the reported numbers, and progress reporting must not
    // be able to break ingestion.
    if let Err(err) = db
        .seed_catchup_floor(
            bridge_id,
            chain_id,
            start_block,
            catchup_cursor,
            realtime_cursor,
        )
        .await
    {
        tracing::warn!(
            err = ?err,
            bridge_id,
            chain_id,
            start_block,
            "failed to seed catchup floor; progress reporting may understate this pair"
        );
    }

    tracing::info!(bridge_id, chain_id, "configured EVM log stream");

    Ok(LogStream::builder(provider)
        .filter(filter)
        .poll_interval(poll_interval)
        .batch_size(batch_size)
        .genesis_block(start_block)
        .realtime_cursor(realtime_cursor)
        .catchup_cursor(catchup_cursor)
        .bridge_id(bridge_id)
        .chain_id(chain_id)
        .head_tag(head_tag)
        .db(Arc::new(db.clone()))
        .catchup()
        .realtime()
        .build()?
        .map(move |batch| (chain_id, batch))
        .boxed())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        log_stream::test_support::{HeadMockService, TaggedHead},
        test_utils::{init_db, mock_db::fill_mock_interchain_database},
    };

    const BRIDGE_ID: i32 = 1;
    const CHAIN_ID: i64 = 1;
    const START_BLOCK: u64 = 10;

    async fn build_with_mock(
        db: &InterchainDatabase,
        mock: &HeadMockService,
        head_tag: Option<HeadBlockTag>,
    ) -> Result<BoxStream<'static, (i64, LogBatch)>> {
        let poll_interval = Duration::from_secs(60);
        match head_tag {
            Some(head_tag) => {
                build_log_stream_for_chain_with_head(
                    mock.provider(),
                    CHAIN_ID,
                    BRIDGE_ID,
                    Filter::new(),
                    START_BLOCK,
                    db,
                    poll_interval,
                    100,
                    head_tag,
                )
                .await
            }
            None => {
                build_log_stream_for_chain(
                    mock.provider(),
                    CHAIN_ID,
                    BRIDGE_ID,
                    Filter::new(),
                    START_BLOCK,
                    db,
                    poll_interval,
                    100,
                )
                .await
            }
        }
    }

    async fn seeded_cursors(db: &InterchainDatabase) -> (i64, i64) {
        let checkpoint = db
            .get_checkpoint(BRIDGE_ID as u64, CHAIN_ID as u64)
            .await
            .expect("checkpoint query")
            .expect("building the stream seeds a checkpoint row");
        (checkpoint.realtime_cursor, checkpoint.catchup_max_cursor)
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_build_log_stream_seeds_checkpoint_from_finalized_head() {
        let guard = init_db("build_log_stream_seeds_from_finalized_head").await;
        fill_mock_interchain_database(&guard).await;
        let db = InterchainDatabase::new(guard.client());
        let mock = HeadMockService::new(1_000, vec![TaggedHead::Block(900)]);

        let _stream = build_with_mock(&db, &mock, Some(HeadBlockTag::Finalized))
            .await
            .expect("stream builds");

        assert_eq!(seeded_cursors(&db).await, (900, 899));
        assert!(
            mock.calls()
                .iter()
                .all(|(method, _)| method != "eth_blockNumber"),
            "a tagged seed must not consult latest: {:?}",
            mock.calls()
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_build_log_stream_latest_tag_seeds_from_latest() {
        let guard = init_db("build_log_stream_latest_tag_seeds_from_latest").await;
        fill_mock_interchain_database(&guard).await;
        let db = InterchainDatabase::new(guard.client());
        let mock = HeadMockService::new(1_000, vec![TaggedHead::Block(900)]);

        let _stream = build_with_mock(&db, &mock, None)
            .await
            .expect("stream builds");

        assert_eq!(seeded_cursors(&db).await, (1_000, 999));
        assert!(
            mock.calls()
                .iter()
                .all(|(method, _)| method != "eth_getBlockByNumber"),
            "the latest seed must not ask for a tagged block: {:?}",
            mock.calls()
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_build_log_stream_fails_when_tagged_head_is_null() {
        let guard = init_db("build_log_stream_fails_when_tagged_head_is_null").await;
        fill_mock_interchain_database(&guard).await;
        let db = InterchainDatabase::new(guard.client());
        let mock = HeadMockService::new(1_000, vec![TaggedHead::Null]);

        let err = build_with_mock(&db, &mock, Some(HeadBlockTag::Finalized))
            .await
            .err()
            .expect("a chain without a finalized block cannot be seeded");

        assert!(
            err.to_string().contains("has no finalized block yet"),
            "unexpected error: {err:#}"
        );
        assert!(
            db.get_checkpoint(BRIDGE_ID as u64, CHAIN_ID as u64)
                .await
                .expect("checkpoint query")
                .is_none(),
            "a failed seed must not write a checkpoint"
        );
    }
}
