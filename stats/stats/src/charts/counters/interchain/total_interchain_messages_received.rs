// SPDX-License-Identifier: LicenseRef-Blockscout

//! Total interchain messages received, within the configured interchain slice.
//!
//! Counts messages admitted by the shared read filter whose destination event
//! was indexed (`dst_tx_hash IS NOT NULL`). When
//! `STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID` is set, "received" additionally
//! means "received *on* the home chain" (`dst_chain_id = home`); with no home
//! chain configured the chart degrades to "destination-side observed", which the
//! server warns about at startup.

use interchain_indexer_entity::crosschain_messages;

use crate::{
    chart_prelude::*,
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterTarget, InterchainFiltered,
    },
};

pub struct TotalInterchainMessagesReceivedStatement;
impl_db_choice!(TotalInterchainMessagesReceivedStatement, UsePrimaryDB);

impl TotalInterchainMessagesReceivedStatement {
    /// Split out from `get_statement_with_context` so tests can render it with an
    /// explicit filter and no `UpdateContext` (hence no database connections).
    fn build(filter: &InterchainFilter) -> Statement {
        use crosschain_messages::Column as C;
        filter
            .messages_query()
            .select_only()
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .filter(C::DstTxHash.is_not_null())
            .apply_if(filter.home_chain_id(), |query, home| {
                query.filter(C::DstChainId.eq(home))
            })
            .build(DbBackend::Postgres)
    }
}

impl StatementFromUpdateTime for TotalInterchainMessagesReceivedStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> Statement {
        Self::build(&cx.interchain_filter)
    }
}

impl InterchainFiltered for TotalInterchainMessagesReceivedStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Messages;
    const CHART_NAME: &'static str = "totalInterchainMessagesReceived";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter)
    }
}

pub type TotalInterchainMessagesReceivedRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalInterchainMessagesReceivedStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInterchainMessagesReceived".into()
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

pub type TotalInterchainMessagesReceived =
    DirectPointLocalDbChartSource<MapToString<TotalInterchainMessagesReceivedRemote>, Properties>;

#[cfg(test)]
mod tests {
    use interchain_indexer_filters::ChainBridgeFilter;

    use super::*;
    use crate::tests::{
        mock_interchain::{
            mock_interchain_horizon, test_interchain_filter, test_interchain_filter_with_horizon,
            test_interchain_home_chain_filter,
        },
        simple_test::simple_test_counter_interchain,
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interchain_messages_received() {
        simple_test_counter_interchain::<TotalInterchainMessagesReceived>(
            "update_total_interchain_messages_received",
            "16",
            None,
            InterchainFilter::default(),
        )
        .await;

        // `(src = 1 OR dst = 1) AND dst = 1 ≡ dst = 1` — the same predicate the
        // deprecated `interchain_primary_id` produced
        simple_test_counter_interchain::<TotalInterchainMessagesReceived>(
            "update_total_interchain_messages_received_primary_1",
            "6",
            None,
            test_interchain_home_chain_filter(1),
        )
        .await;
    }

    /// `counterparty_chain_ids` without a home chain is the within-set
    /// conjunction `src IN {2,3} AND dst IN {2,3}` — no focal `OR` at all.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_msgs_received_counterparties_only() {
        simple_test_counter_interchain::<TotalInterchainMessagesReceived>(
            "total_msgs_received_cp_2_3",
            "2",
            None,
            test_interchain_filter(ChainBridgeFilter {
                counterparty_chain_ids: Some(vec![2, 3]),
                ..Default::default()
            }),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_msgs_received_horizon() {
        simple_test_counter_interchain::<TotalInterchainMessagesReceived>(
            "total_msgs_received_horizon",
            "14",
            None,
            test_interchain_filter_with_horizon(
                ChainBridgeFilter::default(),
                Some(mock_interchain_horizon()),
            ),
        )
        .await;
    }
}
