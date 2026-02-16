use std::ops::Bound;

use crate::{chart_prelude::*, lines::zetachain_cctx_datetime_range_filter};

use zetachain_cctx_entity::sea_orm_active_enums::CctxStatusStatus;

pub struct PendingZetachainCrossChainTxnsStatement;
impl_db_choice!(PendingZetachainCrossChainTxnsStatement, UseZetachainCctxDB);

impl StatementFromUpdateTime for PendingZetachainCrossChainTxnsStatement {
    fn get_statement(
        update_time: DateTime<Utc>,
        _completed_migrations: &IndexerMigrations,
    ) -> sea_orm::Statement {
        zetachain_cctx_entity::cross_chain_tx::Entity::find()
            .select_only()
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .left_join(zetachain_cctx_entity::cctx_status::Entity)
            .filter(zetachain_cctx_entity::cctx_status::Column::Status.is_in([
                CctxStatusStatus::PendingRevert,
                CctxStatusStatus::PendingOutbound,
                CctxStatusStatus::PendingInbound,
            ]))
            .filter(zetachain_cctx_datetime_range_filter(UniversalRange {
                start: None,
                end: Bound::Included(update_time),
            }))
            .build(DbBackend::Postgres)
    }
}

pub type PendingZetachainCrossChainTxnsRemote =
    RemoteDatabaseSource<PullOneNowValue<PendingZetachainCrossChainTxnsStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        // note: not bounded by time
        "pendingZetachainCrossChainTxns".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus::LEAST_RESTRICTIVE
            .with_zetachain_cctx(ZetachainCctxIndexingStatus::IndexedHistoricalData)
    }
}

gettable_const!(Timeout5Secs: Duration = Duration::seconds(5));

pub type PendingZetachainCrossChainTxns = DirectPointCachedLocalDbChartSource<
    MapToString<PendingZetachainCrossChainTxnsRemote>,
    Timeout5Secs,
    Properties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        data_source::{DataSource, UpdateParameters},
        tests::{
            point_construction::dt,
            simple_test::{get_counter, simple_test_counter_with_zetachain_cctx},
        },
    };
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_pending_zetachain_cross_chain_txns() {
        simple_test_counter_with_zetachain_cctx::<PendingZetachainCrossChainTxns>(
            "update_pending_zetachain_cross_chain_txns",
            "1",
            Some(dt("2022-11-11T11:30:00")),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn pending_zetachain_cross_chain_txns_cache_works() {
        // Test cache behavior
        let (db, blockscout, zetachain_cctx) =
            simple_test_counter_with_zetachain_cctx::<PendingZetachainCrossChainTxns>(
                "pending_zetachain_cross_chain_txns_cache_works",
                "1",
                Some(dt("2022-11-11T11:30:00")),
            )
            .await;

        // Add another pending transaction
        crate::tests::mock_zetachain_cctx::insert_cross_chain_txns_with_status(
            &zetachain_cctx,
            [(3, dt("2022-11-11T11:30:01"))],
        )
        .await;

        let mut parameters = UpdateParameters {
            stats_db: &db,
            mode: crate::Mode::Zetachain,
            multichain_filter: None,
            interchain_primary_id: None,
            indexer_db: &blockscout,
            indexer_applied_migrations: IndexerMigrations::latest(),
            second_indexer_db: Some(&zetachain_cctx),
            enabled_update_charts_recursive:
                PendingZetachainCrossChainTxns::all_dependencies_chart_keys(),
            update_time_override: Some(dt("2022-11-11T11:30:00").and_utc()),
            force_full: false,
        };
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        // Query immediately - should return cached value (1)
        let cached_result = get_counter::<PendingZetachainCrossChainTxns>(&cx).await;
        assert_eq!(cached_result.value, "1");

        // Wait for cache timeout (5 seconds)
        parameters.update_time_override = parameters
            .update_time_override
            .map(|t| t + Duration::seconds(1) + Timeout5Secs::get());
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());

        // Query again - should return updated value (2)
        let updated_result = get_counter::<PendingZetachainCrossChainTxns>(&cx).await;
        assert_eq!(updated_result.value, "2");
    }
}
