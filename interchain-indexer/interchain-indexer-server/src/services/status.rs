// SPDX-License-Identifier: LicenseRef-Blockscout

use std::{collections::HashMap, sync::Arc};

use interchain_indexer_entity::indexer_checkpoints;
use interchain_indexer_logic::{
    CrosschainIndexer, InterchainDatabase,
    indexer::progress::{CatchupProgress, CheckpointCursors},
};

use crate::{
    indexers::IndexingTarget,
    proto::{
        ChainIndexingProgress, FullStatus, GetFullStatusRequest, GetIndexingProgressRequest,
        GetIndexingProgressResponse, GetStatusRequest, IndexerStatus, status_service_server::*,
    },
    services::utils::{db_datetime_to_string, map_db_error, sort_json_value},
};

pub struct StatusServiceImpl {
    pub indexers: Vec<Arc<dyn CrosschainIndexer>>,
    db: Arc<InterchainDatabase>,
    targets: Arc<Vec<IndexingTarget>>,
}

impl StatusServiceImpl {
    pub fn new(
        indexers: Vec<Arc<dyn CrosschainIndexer>>,
        db: Arc<InterchainDatabase>,
        targets: Arc<Vec<IndexingTarget>>,
    ) -> Self {
        Self {
            indexers,
            db,
            targets,
        }
    }
}

#[async_trait::async_trait]
impl StatusService for StatusServiceImpl {
    async fn get_full_status(
        &self,
        _request: tonic::Request<GetFullStatusRequest>,
    ) -> Result<tonic::Response<FullStatus>, tonic::Status> {
        Ok(tonic::Response::new(FullStatus {
            indexers: self.indexers.iter().map(get_indexer_status).collect(),
        }))
    }

    async fn get_status_by_indexer_name(
        &self,
        request: tonic::Request<GetStatusRequest>,
    ) -> Result<tonic::Response<IndexerStatus>, tonic::Status> {
        let inner = request.into_inner();
        let indexer = self
            .indexers
            .iter()
            .find(|i| i.name() == inner.indexer_name)
            .ok_or(tonic::Status::not_found(format!(
                "Indexer not found: {}",
                inner.indexer_name
            )))?;

        Ok(tonic::Response::new(get_indexer_status(indexer)))
    }

    async fn get_indexing_progress(
        &self,
        request: tonic::Request<GetIndexingProgressRequest>,
    ) -> Result<tonic::Response<GetIndexingProgressResponse>, tonic::Status> {
        let inner = request.into_inner();
        let items =
            collect_indexing_progress(&self.db, &self.targets, inner.bridge_id, inner.chain_id)
                .await
                .map_err(map_db_error)?;
        Ok(tonic::Response::new(GetIndexingProgressResponse { items }))
    }
}

/// Joins the config-driven `targets` against `indexer_checkpoints` and Part A's
/// `indexer_failure_totals` in Rust, shared by the RPC handler and the periodic
/// metrics worker so the two can never disagree.
///
/// Two reads plus an in-Rust join is intentional, not a shortcut: enumeration is
/// config-driven, so a SQL join **cannot** produce the config-only rows (indexer
/// failed to start, no checkpoint yet) that give this endpoint its main value, and
/// reusing Part A's aggregate avoids a second copy of it.
///
/// Both filters are pushed down to both queries. A filter matching nothing returns
/// an empty list, not an error.
pub(crate) async fn collect_indexing_progress(
    db: &InterchainDatabase,
    targets: &[IndexingTarget],
    bridge_id: Option<i32>,
    chain_id: Option<i64>,
) -> anyhow::Result<Vec<ChainIndexingProgress>> {
    let checkpoints: HashMap<(i32, i64), indexer_checkpoints::Model> = db
        .list_indexer_checkpoints(bridge_id, chain_id)
        .await?
        .into_iter()
        .map(|checkpoint| ((checkpoint.bridge_id, checkpoint.chain_id), checkpoint))
        .collect();

    let failures: HashMap<(i32, i64), u64> = db
        .indexer_failure_totals(bridge_id, chain_id)
        .await?
        .into_iter()
        .map(|(bridge_id, chain_id, blocks, _oldest)| ((bridge_id, chain_id), blocks))
        .collect();

    // `ChainIndexingProgress.chain_id` is a decimal *string* on the wire, so the
    // ordering must be settled on the numeric `i64` before the conversion:
    // sorting the built items would put `{1, 2, 100}` in lexicographic order
    // (`1, 100, 2`).
    let mut selected: Vec<&IndexingTarget> = targets
        .iter()
        .filter(|target| bridge_id.is_none_or(|id| id == target.bridge_id))
        .filter(|target| chain_id.is_none_or(|id| id == target.chain_id))
        .collect();
    selected.sort_by_key(|target| (target.bridge_id, target.chain_id));

    let items: Vec<ChainIndexingProgress> = selected
        .into_iter()
        .map(|target| {
            let key = (target.bridge_id, target.chain_id);
            let checkpoint = checkpoints.get(&key);
            let cursors = checkpoint.map(|checkpoint| CheckpointCursors {
                catchup_min_cursor: checkpoint.validated_catchup_min_cursor(),
                catchup_max_cursor: checkpoint.validated_catchup_cursor(),
                realtime_cursor: checkpoint.validated_realtime_cursor(),
            });
            let progress = CatchupProgress::compute(target.start_block, cursors);
            let failed_blocks = failures.get(&key).copied().unwrap_or(0);
            let checkpoint_updated_at = checkpoint
                .and_then(|checkpoint| checkpoint.updated_at)
                .map(db_datetime_to_string);

            ChainIndexingProgress {
                bridge_id: target.bridge_id,
                chain_id: target.chain_id.to_string(),
                start_block: progress.start_block,
                catchup_min_cursor: progress.catchup_min_cursor,
                catchup_max_cursor: progress.catchup_max_cursor,
                realtime_cursor: progress.realtime_cursor,
                catchup_complete: progress.catchup_complete(failed_blocks),
                catchup_progress_percent: progress.progress_percent,
                catchup_blocks_remaining: progress.blocks_remaining,
                failed_blocks,
                checkpoint_updated_at,
            }
        })
        .collect();

    Ok(items)
}

fn get_indexer_status(indexer: &Arc<dyn CrosschainIndexer>) -> IndexerStatus {
    let status = indexer.get_status();
    IndexerStatus {
        name: indexer.name(),
        description: (!indexer.description().is_empty()).then_some(indexer.description()),
        state: status.state.to_string(),
        init_timestamp: db_datetime_to_string(status.init_timestamp),
        extra_info: {
            let json = serde_json::Value::Object(status.extra_info.into_iter().collect());
            let json = sort_json_value(json);
            serde_json::from_value::<prost_wkt_types::Struct>(json).ok()
        },
    }
}

#[cfg(test)]
mod tests {
    use blockscout_service_launcher::test_database::TestDbGuard;
    use interchain_indexer_entity::{bridges, chains};
    use interchain_indexer_logic::indexer::failure_ledger::BlockRange;
    use sea_orm::ActiveValue::Set;

    use super::*;

    async fn init_db(name: &str) -> TestDbGuard {
        TestDbGuard::new::<migration::Migrator>(name).await
    }

    async fn seed_bridges_and_chains(db: &InterchainDatabase) {
        db.upsert_bridges(vec![
            bridges::ActiveModel {
                id: Set(1),
                name: Set("bridge-1".to_string()),
                enabled: Set(true),
                ..Default::default()
            },
            bridges::ActiveModel {
                id: Set(2),
                name: Set("bridge-2".to_string()),
                enabled: Set(true),
                ..Default::default()
            },
        ])
        .await
        .unwrap();
        db.upsert_chains(vec![
            chains::ActiveModel {
                id: Set(1),
                name: Set("chain-1".to_string()),
                ..Default::default()
            },
            chains::ActiveModel {
                id: Set(100),
                name: Set("chain-100".to_string()),
                ..Default::default()
            },
        ])
        .await
        .unwrap();
    }

    /// `failed_blocks` couples to Part A's `record_indexer_failures`
    /// disjointness guarantee: `collect_indexing_progress` just sums
    /// `indexer_failure_totals`, and that sum is exact only because Part A's
    /// writer keeps a pair's rows disjoint and non-adjacent
    /// (`record_indexer_failures_disjointness_holds_after_mixed_merges` in
    /// `database.rs` is the test that actually proves that property).
    #[tokio::test]
    #[ignore = "needs database"]
    async fn collect_indexing_progress_sums_disjoint_indexer_failures_rows() {
        let test_db = init_db("collect_progress_failed_blocks_sum").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridges_and_chains(&db).await;

        db.seed_catchup_floor(1, 1, 1000, 999, 2000).await.unwrap();
        db.record_indexer_failures(
            1,
            1,
            &[(
                BlockRange {
                    from: 1_100,
                    to: 1_199,
                },
                "boom".to_string(),
            )],
        )
        .await
        .unwrap();
        // A real gap, so this stays a disjoint second row (same reasoning as
        // Part A's `indexer_failure_totals` test).
        db.record_indexer_failures(
            1,
            1,
            &[(
                BlockRange {
                    from: 1_500,
                    to: 1_599,
                },
                "boom2".to_string(),
            )],
        )
        .await
        .unwrap();

        let targets = [IndexingTarget {
            bridge_id: 1,
            chain_id: 1,
            start_block: 1000,
        }];

        let items = collect_indexing_progress(&db, &targets, None, None)
            .await
            .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].failed_blocks, 200,
            "100 + 100 across two disjoint rows"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn collect_indexing_progress_reports_zero_failed_blocks_with_no_rows() {
        let test_db = init_db("collect_progress_no_failure_rows").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridges_and_chains(&db).await;
        db.seed_catchup_floor(1, 1, 1000, 999, 2000).await.unwrap();

        let targets = [IndexingTarget {
            bridge_id: 1,
            chain_id: 1,
            start_block: 1000,
        }];

        let items = collect_indexing_progress(&db, &targets, None, None)
            .await
            .unwrap();
        assert_eq!(items[0].failed_blocks, 0);
    }

    /// `catchup_complete` is the one field that reads *both* records, so it
    /// is also the one field a broken join would silently get wrong: the
    /// pure predicate is unit-tested in `progress.rs`, but only a DB test
    /// proves the failure ledger's aggregate actually reaches it. Both pairs
    /// here are fully scanned (`lo = 1000 > catchup_max_cursor = 999`,
    /// `realtime_cursor = 2000`), so the cursors are identical and the open
    /// hole is the only difference between them.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn collect_indexing_progress_catchup_complete_requires_a_clean_failure_ledger() {
        let test_db = init_db("collect_progress_catchup_complete").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridges_and_chains(&db).await;

        db.seed_catchup_floor(1, 1, 1000, 999, 2000).await.unwrap();
        db.seed_catchup_floor(2, 100, 1000, 999, 2000)
            .await
            .unwrap();
        db.record_indexer_failures(
            1,
            1,
            &[(
                BlockRange {
                    from: 1_100,
                    to: 1_199,
                },
                "boom".to_string(),
            )],
        )
        .await
        .unwrap();

        let targets = [
            IndexingTarget {
                bridge_id: 1,
                chain_id: 1,
                start_block: 1000,
            },
            IndexingTarget {
                bridge_id: 2,
                chain_id: 100,
                start_block: 1000,
            },
        ];

        let items = collect_indexing_progress(&db, &targets, None, None)
            .await
            .unwrap();
        assert_eq!(items.len(), 2);

        assert_eq!(items[0].catchup_progress_percent, 100.0);
        assert_eq!(items[0].failed_blocks, 100);
        assert!(
            !items[0].catchup_complete,
            "100% scanned with an open hole is not a complete catch-up"
        );

        assert_eq!(items[1].catchup_progress_percent, 100.0);
        assert_eq!(items[1].failed_blocks, 0);
        assert!(
            items[1].catchup_complete,
            "100% scanned with an empty failure ledger is a complete catch-up"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn collect_indexing_progress_pair_with_no_checkpoint_row_reports_zero_and_absent_updated_at()
     {
        let test_db = init_db("collect_progress_no_checkpoint_row").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridges_and_chains(&db).await;
        // Deliberately no `seed_catchup_floor` call: the indexer for this
        // pair failed to start, so no checkpoint row exists.

        let targets = [IndexingTarget {
            bridge_id: 1,
            chain_id: 1,
            start_block: 1000,
        }];

        let items = collect_indexing_progress(&db, &targets, None, None)
            .await
            .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].catchup_progress_percent, 0.0);
        assert!(!items[0].catchup_complete);
        assert_eq!(
            items[0].checkpoint_updated_at, None,
            "absent checkpoint_updated_at is the payload's 'no state at all' marker"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn collect_indexing_progress_pushes_both_filters_down_to_both_queries() {
        let test_db = init_db("collect_progress_filters_pushed_down").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridges_and_chains(&db).await;

        db.seed_catchup_floor(1, 1, 1000, 999, 2000).await.unwrap();
        db.seed_catchup_floor(2, 100, 1000, 999, 2000)
            .await
            .unwrap();
        db.record_indexer_failures(
            1,
            1,
            &[(
                BlockRange {
                    from: 1_100,
                    to: 1_199,
                },
                "boom".to_string(),
            )],
        )
        .await
        .unwrap();
        db.record_indexer_failures(
            2,
            100,
            &[(
                BlockRange {
                    from: 1_100,
                    to: 1_199,
                },
                "boom".to_string(),
            )],
        )
        .await
        .unwrap();

        let targets = [
            IndexingTarget {
                bridge_id: 1,
                chain_id: 1,
                start_block: 1000,
            },
            IndexingTarget {
                bridge_id: 2,
                chain_id: 100,
                start_block: 1000,
            },
        ];

        let items = collect_indexing_progress(&db, &targets, Some(1), Some(1))
            .await
            .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!((items[0].bridge_id, items[0].chain_id.as_str()), (1, "1"));
        assert_eq!(items[0].failed_blocks, 100);

        let none = collect_indexing_progress(&db, &targets, Some(1), Some(100))
            .await
            .unwrap();
        assert!(
            none.is_empty(),
            "a filter matching no configured target must return an empty list, not an error"
        );
    }

    /// `ChainIndexingProgress.chain_id` is a decimal string on the wire, so
    /// ordering must be settled on the numeric `i64` before the conversion.
    /// Chain ids `{1, 2, 100}` are the smallest set where a lexicographic sort
    /// (`1, 100, 2`) and a numeric one disagree; sorting the built proto items
    /// instead of the targets fails this test.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_collect_indexing_progress_orders_chain_ids_numerically() {
        let test_db = init_db("collect_progress_numeric_chain_id_order").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridges_and_chains(&db).await;
        db.upsert_chains(vec![chains::ActiveModel {
            id: Set(2),
            name: Set("chain-2".to_string()),
            ..Default::default()
        }])
        .await
        .unwrap();

        // Declared out of numeric order so the ordering cannot come from the
        // input sequence.
        let targets = [
            IndexingTarget {
                bridge_id: 1,
                chain_id: 100,
                start_block: 1000,
            },
            IndexingTarget {
                bridge_id: 1,
                chain_id: 1,
                start_block: 1000,
            },
            IndexingTarget {
                bridge_id: 1,
                chain_id: 2,
                start_block: 1000,
            },
        ];

        let items = collect_indexing_progress(&db, &targets, None, None)
            .await
            .unwrap();
        let chain_ids: Vec<&str> = items.iter().map(|item| item.chain_id.as_str()).collect();
        assert_eq!(
            chain_ids,
            vec!["1", "2", "100"],
            "chain ids must be ordered numerically, not lexicographically; got {chain_ids:?}"
        );
    }
}
