// SPDX-License-Identifier: LicenseRef-Blockscout

//! Total interchain transfers received, within the configured interchain slice.
//!
//! Counts transfers admitted by the shared read filter whose parent message's
//! destination event was indexed (`crosschain_messages.dst_tx_hash IS NOT NULL`).
//!
//! The join to `crosschain_messages` exists **only** to reach `dst_tx_hash`; the
//! filter and the directional term both stay on the transfer's own token
//! columns — see [`super::TotalInterchainTransfersSent`]'s module docs for why.
//! The join is composite (`(message_id, bridge_id)`) via the declared SeaORM
//! relation.

use interchain_indexer_entity::{crosschain_messages, crosschain_transfers};

use crate::{
    chart_prelude::*,
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterTarget, InterchainFiltered,
    },
};

pub struct TotalInterchainTransfersReceivedStatement;
impl_db_choice!(TotalInterchainTransfersReceivedStatement, UsePrimaryDB);

impl TotalInterchainTransfersReceivedStatement {
    /// Split out from `get_statement_with_context` so tests can render it with an
    /// explicit filter and no `UpdateContext` (hence no database connections).
    fn build(filter: &InterchainFilter) -> Statement {
        filter
            .transfers_joined_query()
            .select_only()
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .filter(crosschain_messages::Column::DstTxHash.is_not_null())
            .apply_if(filter.home_chain_id(), |query, home| {
                query.filter(crosschain_transfers::Column::TokenDstChainId.eq(home))
            })
            .build(DbBackend::Postgres)
    }
}

impl StatementFromUpdateTime for TotalInterchainTransfersReceivedStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> Statement {
        Self::build(&cx.interchain_filter)
    }
}

impl InterchainFiltered for TotalInterchainTransfersReceivedStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Transfers;
    const CHART_NAME: &'static str = "totalInterchainTransfersReceived";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter)
    }
}

pub type TotalInterchainTransfersReceivedRemote = RemoteDatabaseSource<
    PullOneNowValue<TotalInterchainTransfersReceivedStatement, NaiveDate, i64>,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInterchainTransfersReceived".into()
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

pub type TotalInterchainTransfersReceived =
    DirectPointLocalDbChartSource<MapToString<TotalInterchainTransfersReceivedRemote>, Properties>;

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
    async fn update_total_interchain_transfers_received() {
        simple_test_counter_interchain::<TotalInterchainTransfersReceived>(
            "update_total_interchain_transfers_received",
            "24",
            None,
            InterchainFilter::default(),
        )
        .await;

        simple_test_counter_interchain::<TotalInterchainTransfersReceived>(
            "update_total_interchain_transfers_received_primary_1",
            "6",
            None,
            test_interchain_home_chain_filter(1),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_transfers_received_filtered() {
        simple_test_counter_interchain::<TotalInterchainTransfersReceived>(
            "total_transfers_received_cp_2_3",
            "5",
            None,
            test_interchain_filter(ChainBridgeFilter {
                counterparty_chain_ids: Some(vec![2, 3]),
                ..Default::default()
            }),
        )
        .await;

        simple_test_counter_interchain::<TotalInterchainTransfersReceived>(
            "total_transfers_received_horizon",
            "20",
            None,
            test_interchain_filter_with_horizon(
                ChainBridgeFilter::default(),
                Some(mock_interchain_horizon()),
            ),
        )
        .await;
    }
}
