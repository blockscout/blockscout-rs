// SPDX-License-Identifier: LicenseRef-Blockscout

//! Total interchain messages sent, within the configured interchain slice.
//!
//! Counts messages admitted by the shared read filter whose source event was
//! indexed (`src_tx_hash IS NOT NULL`). When
//! `STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID` is set, "sent" additionally means
//! "sent *from* the home chain" (`src_chain_id = home`); with no home chain
//! configured the chart degrades to "source-side observed", which the server
//! warns about at startup.

use interchain_indexer_entity::crosschain_messages;

use crate::{
    chart_prelude::*,
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterTarget, InterchainFiltered,
    },
};

pub struct TotalInterchainMessagesSentStatement;
impl_db_choice!(TotalInterchainMessagesSentStatement, UsePrimaryDB);

impl TotalInterchainMessagesSentStatement {
    /// Split out from `get_statement_with_context` so tests can render it with an
    /// explicit filter and no `UpdateContext` (hence no database connections).
    fn build(filter: &InterchainFilter) -> Statement {
        use crosschain_messages::Column as C;
        filter
            .messages_query()
            .select_only()
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .filter(C::SrcTxHash.is_not_null())
            // the directional term belongs to the chart, not to the filter
            .apply_if(filter.home_chain_id(), |query, home| {
                query.filter(C::SrcChainId.eq(home))
            })
            .build(DbBackend::Postgres)
    }
}

impl StatementFromUpdateTime for TotalInterchainMessagesSentStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> Statement {
        Self::build(&cx.interchain_filter)
    }
}

impl InterchainFiltered for TotalInterchainMessagesSentStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Messages;
    const CHART_NAME: &'static str = "totalInterchainMessagesSent";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter)
    }
}

pub type TotalInterchainMessagesSentRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalInterchainMessagesSentStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInterchainMessagesSent".into()
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
        IndexingStatus::LEAST_RESTRICTIVE.with_interchain(InterchainIndexingStatus::CaughtUp)
    }
}

pub type TotalInterchainMessagesSent =
    DirectPointLocalDbChartSource<MapToString<TotalInterchainMessagesSentRemote>, Properties>;

#[cfg(test)]
mod tests {
    use interchain_indexer_filters::ChainBridgeFilter;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        charts::db_interaction::read::{find_chart, get_min_date, recorded_min_chart_date},
        data_source::{
            source::DataSource,
            types::{IndexerMigrations, UpdateContext, UpdateParameters},
        },
        tests::{
            mock_interchain::{
                mock_interchain_horizon, test_interchain_filter,
                test_interchain_filter_with_horizon, test_interchain_home_chain_filter,
            },
            normalize_sql,
            point_construction::d,
            simple_test::{
                prepare_interchain_chart_test, simple_test_counter_interchain,
                update_and_query_interchain_counter,
            },
        },
    };

    #[test]
    fn statement_is_correct() {
        let actual =
            TotalInterchainMessagesSentStatement::build(&test_interchain_home_chain_filter(1));
        let expected = r#"
            SELECT COUNT(*) AS "value" FROM "crosschain_messages"
            WHERE ("crosschain_messages"."src_chain_id" = 1
                   OR "crosschain_messages"."dst_chain_id" = 1)
              AND "crosschain_messages"."src_tx_hash" IS NOT NULL
              AND "crosschain_messages"."src_chain_id" = 1
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()))
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interchain_messages_sent() {
        simple_test_counter_interchain::<TotalInterchainMessagesSent>(
            "update_total_interchain_messages_sent",
            "20",
            None,
            InterchainFilter::default(),
        )
        .await;

        // `(src = 1 OR dst = 1) AND src = 1 ≡ src = 1` — the same predicate the
        // deprecated `interchain_primary_id` produced
        simple_test_counter_interchain::<TotalInterchainMessagesSent>(
            "update_total_interchain_messages_sent_primary_1",
            "15",
            None,
            test_interchain_home_chain_filter(1),
        )
        .await;
    }

    /// A counter is a [`DirectPointLocalDbChartSource`]: it recomputes its single
    /// point from scratch on every update, so it is self-healing across a filter
    /// change even without the clear-on-fingerprint-change. This asserts the new
    /// clear path did not *break* that while fixing the line charts — each value
    /// is the one [`simple_test_counter_interchain`] computes from scratch for
    /// the same filter (`20` unfiltered, `15` with `home_chain_id = 1`).
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_interchain_messages_sent_filter_change_is_self_healing() {
        let (init_time, db, indexer) =
            prepare_interchain_chart_test::<TotalInterchainMessagesSent>(
                "total_msgs_sent_filter_change",
            )
            .await;
        for (offset_seconds, filter, expected) in [
            (0, InterchainFilter::default(), "20"),
            (1, test_interchain_home_chain_filter(1), "15"),
            (2, InterchainFilter::default(), "20"),
        ] {
            assert_eq!(
                update_and_query_interchain_counter::<TotalInterchainMessagesSent>(
                    &db,
                    &indexer,
                    filter,
                    init_time + chrono::TimeDelta::seconds(offset_seconds),
                )
                .await,
                expected
            );
        }
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_interchain_messages_sent_directional() {
        // `dst_chain_ids` alone emits no focal `OR` and no `src = home` term, so
        // this is a plain `dst IN (1) AND src_tx_hash IS NOT NULL`
        simple_test_counter_interchain::<TotalInterchainMessagesSent>(
            "total_msgs_sent_dst_1",
            "3",
            None,
            test_interchain_filter(ChainBridgeFilter {
                dst_chain_ids: Some(vec![1]),
                ..Default::default()
            }),
        )
        .await;

        simple_test_counter_interchain::<TotalInterchainMessagesSent>(
            "total_msgs_sent_horizon",
            "16",
            None,
            test_interchain_filter_with_horizon(
                ChainBridgeFilter::default(),
                Some(mock_interchain_horizon()),
            ),
        )
        .await;
    }

    /// The `ChartType::Line` gate, demonstrated rather than merely asserted
    /// correct: a counter's own stored floor (its single point, always stamped
    /// at the *current* date) regresses relative to the indexer's floor exactly
    /// like a line chart's would. Without the gate, this comparison would fire
    /// on this counter — and every other interchain counter — every cycle,
    /// forever. A test that only checked "the counter's value is right anyway"
    /// would pass with or without the gate; this one would not.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn counter_floor_would_regress_forever_without_the_line_gate() {
        let (init_time, db, indexer) =
            prepare_interchain_chart_test::<TotalInterchainMessagesSent>(
                "counter_floor_would_regress_forever",
            )
            .await;
        update_and_query_interchain_counter::<TotalInterchainMessagesSent>(
            &db,
            &indexer,
            InterchainFilter::default(),
            init_time,
        )
        .await;

        let chart_id = find_chart(&db, &Properties::key())
            .await
            .unwrap()
            .expect("chart must exist after the update");
        let stored_floor = recorded_min_chart_date(&db, chart_id).await.unwrap();
        assert_eq!(
            stored_floor,
            Some(init_time.date_naive()),
            "a counter's stored floor is always the date it was last computed at"
        );

        let params = UpdateParameters {
            stats_db: &db,
            mode: crate::Mode::Interchain,
            multichain_filter: None,
            interchain_filter: InterchainFilter::default(),
            indexer_db: &indexer,
            second_indexer_db: None,
            indexer_applied_migrations: IndexerMigrations::latest(),
            enabled_update_charts_recursive:
                TotalInterchainMessagesSent::all_dependencies_chart_keys(),
            update_time_override: Some(init_time),
            force_full: false,
        };
        let cx = UpdateContext::from_params_now_or_override(params);
        let indexer_floor = get_min_date(&cx).await.unwrap().date();
        assert_eq!(
            indexer_floor,
            d("2022-12-20"),
            "the indexer's true filtered floor is the earliest fixture date"
        );

        assert!(
            stored_floor.unwrap() > indexer_floor,
            "the counter's floor regresses relative to the indexer's floor exactly like a \
             line chart's would — this is the condition the ChartType::Line gate exists to \
             ignore for counters"
        );
    }
}
