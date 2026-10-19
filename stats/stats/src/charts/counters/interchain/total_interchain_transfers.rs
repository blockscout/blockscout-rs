// SPDX-License-Identifier: LicenseRef-Blockscout

//! Total interchain transfers **within the configured interchain slice**.
//!
//! Counts `crosschain_transfers` rows admitted by the shared read filter, which
//! is evaluated on the transfer's own `token_src_chain_id` /
//! `token_dst_chain_id` / `bridge_id`. There is deliberately **no join** to
//! `crosschain_messages`: the predicate needs nothing from it, and the chart has
//! neither a time axis nor an observability term.

use crate::{
    chart_prelude::*,
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterTarget, InterchainFiltered,
    },
};

pub struct TotalInterchainTransfersStatement;
impl_db_choice!(TotalInterchainTransfersStatement, UsePrimaryDB);

impl TotalInterchainTransfersStatement {
    /// Split out from `get_statement_with_context` so tests can render it with an
    /// explicit filter and no `UpdateContext` (hence no database connections).
    fn build(filter: &InterchainFilter) -> Statement {
        filter
            .transfers_query()
            .select_only()
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .build(DbBackend::Postgres)
    }
}

impl StatementFromUpdateTime for TotalInterchainTransfersStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> Statement {
        Self::build(&cx.interchain_filter)
    }
}

impl InterchainFiltered for TotalInterchainTransfersStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Transfers;
    const CHART_NAME: &'static str = "totalInterchainTransfers";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter)
    }
}

pub type TotalInterchainTransfersRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalInterchainTransfersStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInterchainTransfers".into()
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

pub type TotalInterchainTransfers =
    DirectPointLocalDbChartSource<MapToString<TotalInterchainTransfersRemote>, Properties>;

#[cfg(test)]
mod tests {
    use interchain_indexer_filters::ChainBridgeFilter;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::tests::{
        mock_interchain::{
            MOCK_SECOND_BRIDGE_ID, mock_interchain_horizon, test_interchain_filter,
            test_interchain_filter_with_horizon, test_interchain_home_chain_filter,
        },
        normalize_sql,
        simple_test::simple_test_counter_interchain,
    };

    #[test]
    fn statement_is_correct() {
        let actual =
            TotalInterchainTransfersStatement::build(&test_interchain_home_chain_filter(1));
        let expected = r#"
            SELECT COUNT(*) AS "value" FROM "crosschain_transfers"
            WHERE "crosschain_transfers"."token_src_chain_id" = 1
               OR "crosschain_transfers"."token_dst_chain_id" = 1
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()))
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interchain_transfers() {
        simple_test_counter_interchain::<TotalInterchainTransfers>(
            "update_total_interchain_transfers",
            "47",
            None,
            InterchainFilter::default(),
        )
        .await;

        // redefined in place: this chart used to ignore filtering entirely
        simple_test_counter_interchain::<TotalInterchainTransfers>(
            "total_interchain_transfers_home_1",
            "40",
            None,
            test_interchain_home_chain_filter(1),
        )
        .await;
    }

    /// With no join to `crosschain_messages`, the id reused across the two
    /// bridges cannot fan out: the count is exactly the number of transfer rows.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn transfers_do_not_fan_out_across_bridges() {
        simple_test_counter_interchain::<TotalInterchainTransfers>(
            "transfers_no_fan_out_bridge_2",
            "3",
            None,
            test_interchain_filter(ChainBridgeFilter {
                bridge_ids: Some(vec![MOCK_SECOND_BRIDGE_ID]),
                ..Default::default()
            }),
        )
        .await;
    }

    /// The horizon excludes message 23's transfer (token chains `1 → 4`), message
    /// 24's transfer (`3 → 4`) and all three bridge-2 transfers. Message 22's
    /// transfer survives: `transfers_condition` has no NULL guard, because both
    /// token chain columns are NOT NULL.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_interchain_transfers_horizon() {
        simple_test_counter_interchain::<TotalInterchainTransfers>(
            "total_interchain_transfers_horizon",
            "42",
            None,
            test_interchain_filter_with_horizon(
                ChainBridgeFilter::default(),
                Some(mock_interchain_horizon()),
            ),
        )
        .await;
    }
}
