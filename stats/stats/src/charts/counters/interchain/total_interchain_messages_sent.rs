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
        IndexingStatus::LEAST_RESTRICTIVE
    }
}

pub type TotalInterchainMessagesSent =
    DirectPointLocalDbChartSource<MapToString<TotalInterchainMessagesSentRemote>, Properties>;

#[cfg(test)]
mod tests {
    use interchain_indexer_filters::ChainBridgeFilter;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::tests::{
        mock_interchain::{
            mock_interchain_horizon, test_interchain_filter, test_interchain_filter_with_horizon,
            test_interchain_home_chain_filter,
        },
        normalize_sql,
        simple_test::simple_test_counter_interchain,
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
}
