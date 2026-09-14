// SPDX-License-Identifier: LicenseRef-Blockscout

//! Total interchain transfers sent, within the configured interchain slice.
//!
//! Counts transfers admitted by the shared read filter whose parent message's
//! source event was indexed (`crosschain_messages.src_tx_hash IS NOT NULL`).
//!
//! The join to `crosschain_messages` exists **only** to reach `src_tx_hash` — an
//! observability fact that lives on the message and nowhere else. The filter
//! stays on the transfer's own token columns, and so does the directional term:
//! when `STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID` is set, "sent" means
//! `token_src_chain_id = home`, not `crosschain_messages.src_chain_id = home`.
//!
//! That choice is deliberate, and it is the one place in this family where the
//! two differ. Reasons, in order:
//!
//! 1. Dropping the directional term altogether (the alternative considered)
//!    would silently *widen* this chart relative to its previous definition
//!    whenever a home chain is configured.
//! 2. Keeping it on the message's route would make this the only transfer chart
//!    whose filtering mixes the two tables' chain columns, which is exactly the
//!    ambiguity the shared filter exists to remove.
//! 3. Token chains equal the message route for the overwhelming majority of
//!    rows, so the numbers stay as close to the previous definition as parity
//!    allows.
//!
//! The join is composite — `(message_id, bridge_id)` — via the declared SeaORM
//! relation. A `message_id`-only join fans transfers out across bridges that
//! happen to reuse a numeric message id.

use interchain_indexer_entity::{crosschain_messages, crosschain_transfers};

use crate::{
    chart_prelude::*,
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterTarget, InterchainFiltered,
    },
};

pub struct TotalInterchainTransfersSentStatement;
impl_db_choice!(TotalInterchainTransfersSentStatement, UsePrimaryDB);

impl TotalInterchainTransfersSentStatement {
    /// Split out from `get_statement_with_context` so tests can render it with an
    /// explicit filter and no `UpdateContext` (hence no database connections).
    fn build(filter: &InterchainFilter) -> Statement {
        filter
            .transfers_joined_query()
            .select_only()
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .filter(crosschain_messages::Column::SrcTxHash.is_not_null())
            .apply_if(filter.home_chain_id(), |query, home| {
                query.filter(crosschain_transfers::Column::TokenSrcChainId.eq(home))
            })
            .build(DbBackend::Postgres)
    }
}

impl StatementFromUpdateTime for TotalInterchainTransfersSentStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> Statement {
        Self::build(&cx.interchain_filter)
    }
}

impl InterchainFiltered for TotalInterchainTransfersSentStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Transfers;
    const CHART_NAME: &'static str = "totalInterchainTransfersSent";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter)
    }
}

pub type TotalInterchainTransfersSentRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalInterchainTransfersSentStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInterchainTransfersSent".into()
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

pub type TotalInterchainTransfersSent =
    DirectPointLocalDbChartSource<MapToString<TotalInterchainTransfersSentRemote>, Properties>;

#[cfg(test)]
mod tests {
    use interchain_indexer_filters::ChainBridgeFilter;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::tests::{
        mock_interchain::{
            mock_interchain_horizon, test_interchain_filter_with_horizon,
            test_interchain_home_chain_filter,
        },
        normalize_sql,
        simple_test::simple_test_counter_interchain,
    };

    #[test]
    fn statement_is_correct() {
        let actual =
            TotalInterchainTransfersSentStatement::build(&test_interchain_home_chain_filter(1));
        let expected = r#"
            SELECT COUNT(*) AS "value" FROM "crosschain_transfers"
            INNER JOIN "crosschain_messages"
                ON "crosschain_transfers"."message_id" = "crosschain_messages"."id"
               AND "crosschain_transfers"."bridge_id" = "crosschain_messages"."bridge_id"
            WHERE ("crosschain_transfers"."token_src_chain_id" = 1
                   OR "crosschain_transfers"."token_dst_chain_id" = 1)
              AND "crosschain_messages"."src_tx_hash" IS NOT NULL
              AND "crosschain_transfers"."token_src_chain_id" = 1
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()))
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interchain_transfers_sent() {
        simple_test_counter_interchain::<TotalInterchainTransfersSent>(
            "update_total_interchain_transfers_sent",
            "43",
            None,
            InterchainFilter::default(),
        )
        .await;

        // message 24's transfer (token chains `3 → 4` on a `1 → 2` route) is the
        // one row this differs by from the previous `m.src_chain_id = 1` shape
        simple_test_counter_interchain::<TotalInterchainTransfersSent>(
            "update_total_interchain_transfers_sent_primary_1",
            "27",
            None,
            test_interchain_home_chain_filter(1),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_transfers_sent_horizon() {
        simple_test_counter_interchain::<TotalInterchainTransfersSent>(
            "total_transfers_sent_horizon",
            "38",
            None,
            test_interchain_filter_with_horizon(
                ChainBridgeFilter::default(),
                Some(mock_interchain_horizon()),
            ),
        )
        .await;
    }
}
