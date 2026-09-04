// SPDX-License-Identifier: LicenseRef-Blockscout

//! Total interchain messages **within the configured interchain slice**.
//!
//! Counts `crosschain_messages` rows admitted by the shared read filter
//! (`STATS__INTERCHAIN_FILTER__*` plus the observability horizon). The chart
//! itself adds no term of its own: it is the slice's size, not an
//! observability statement, so neither `src_tx_hash` nor `dst_tx_hash` is
//! consulted.

use crate::{
    chart_prelude::*,
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterTarget, InterchainFiltered,
    },
};

pub struct TotalInterchainMessagesStatement;
impl_db_choice!(TotalInterchainMessagesStatement, UsePrimaryDB);

impl TotalInterchainMessagesStatement {
    /// Split out from `get_statement_with_context` so tests can render it with an
    /// explicit filter and no `UpdateContext` (hence no database connections).
    fn build(filter: &InterchainFilter) -> Statement {
        filter
            .messages_query()
            .select_only()
            // `PullOneNowValue<_, _, i64>` reads a column named `value`, and
            // Postgres `COUNT(*)` is already `bigint` — no cast.
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .build(DbBackend::Postgres)
    }
}

impl StatementFromUpdateTime for TotalInterchainMessagesStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> Statement {
        Self::build(&cx.interchain_filter)
    }
}

impl InterchainFiltered for TotalInterchainMessagesStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Messages;
    const CHART_NAME: &'static str = "totalInterchainMessages";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter)
    }
}

pub type TotalInterchainMessagesRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalInterchainMessagesStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInterchainMessages".into()
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

pub type TotalInterchainMessages =
    DirectPointLocalDbChartSource<MapToString<TotalInterchainMessagesRemote>, Properties>;

#[cfg(test)]
mod tests {
    use interchain_indexer_filters::ChainBridgeFilter;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::tests::{
        mock_interchain::{
            MOCK_BRIDGE_ID, MOCK_SECOND_BRIDGE_ID, mock_interchain_horizon, test_interchain_filter,
            test_interchain_filter_with_horizon, test_interchain_home_chain_filter,
        },
        normalize_sql,
        simple_test::simple_test_counter_interchain,
    };

    #[test]
    fn statement_is_correct() {
        let actual =
            TotalInterchainMessagesStatement::build(&test_interchain_filter(ChainBridgeFilter {
                home_chain_id: Some(1),
                counterparty_chain_ids: Some(vec![2, 3]),
                ..Default::default()
            }));
        // note the focal `OR` is one parenthesised term of the outer `AND`
        let expected = r#"
            SELECT COUNT(*) AS "value" FROM "crosschain_messages"
            WHERE ("crosschain_messages"."src_chain_id" = 1
                       AND "crosschain_messages"."dst_chain_id" IN (2, 3))
               OR ("crosschain_messages"."dst_chain_id" = 1
                       AND "crosschain_messages"."src_chain_id" IN (2, 3))
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()))
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interchain_messages() {
        simple_test_counter_interchain::<TotalInterchainMessages>(
            "update_total_interchain_messages",
            "26",
            None,
            InterchainFilter::default(),
        )
        .await;

        // redefined in place: this chart used to ignore filtering entirely
        simple_test_counter_interchain::<TotalInterchainMessages>(
            "total_interchain_messages_home_1",
            "23",
            None,
            test_interchain_home_chain_filter(1),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_interchain_messages_bridge_ids() {
        simple_test_counter_interchain::<TotalInterchainMessages>(
            "total_interchain_messages_bridge_1",
            "24",
            None,
            test_interchain_filter(ChainBridgeFilter {
                bridge_ids: Some(vec![MOCK_BRIDGE_ID]),
                ..Default::default()
            }),
        )
        .await;

        simple_test_counter_interchain::<TotalInterchainMessages>(
            "total_interchain_messages_bridge_2",
            "2",
            None,
            test_interchain_filter(ChainBridgeFilter {
                bridge_ids: Some(vec![MOCK_SECOND_BRIDGE_ID]),
                ..Default::default()
            }),
        )
        .await;
    }

    /// The horizon excludes message 22 (NULL destination), message 23 (`dst = 4`,
    /// outside bridge 1's contract chains) and both bridge-2 messages (bridge 2 is
    /// listed with an empty chain set, so its own disjunct renders `1 = 2` and the
    /// permissive arm cannot admit it either).
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_interchain_messages_horizon() {
        simple_test_counter_interchain::<TotalInterchainMessages>(
            "total_interchain_messages_horizon",
            "22",
            None,
            test_interchain_filter_with_horizon(
                ChainBridgeFilter::default(),
                Some(mock_interchain_horizon()),
            ),
        )
        .await;
    }
}
