// SPDX-License-Identifier: LicenseRef-Blockscout

//! Total unique interchain transfer users within the configured interchain
//! slice — distinct `sender_address` and `recipient_address` over the transfers
//! the shared read filter admits.
//!
//! Two properties of this counter are deliberate:
//!
//! - **No join to `crosschain_messages`.** `transfers_condition()` needs nothing
//!   from it, and there is no time axis and no observability term here. (The
//!   previous version joined in its filtered arm and not in its unfiltered one;
//!   both arms are now one shape.) That also removes this counter's exposure to
//!   the `message_id`-only fan-out rather than merely fixing it.
//! - **No directional term**, even when
//!   `STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID` is set: a user is a user
//!   regardless of which side of the route they were on.
//!
//! The `UNION` is kept rather than rewritten to `unnest` + `COUNT(DISTINCT …)`:
//! a `UNION` de-duplicates, so `COUNT(*)` over it *is* a distinct count, and
//! keeping the shape makes this a pure representation change. It is also the
//! reason the filter is applied **twice** here — once per arm — which
//! `EXPECTED_APPLICATIONS` declares.

use interchain_indexer_entity::crosschain_transfers;
use sea_query::{Query, UnionType};

use crate::{
    chart_prelude::*,
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterTarget, InterchainFiltered,
    },
};

pub struct TotalInterchainTransferUsersStatement;
impl_db_choice!(TotalInterchainTransferUsersStatement, UsePrimaryDB);

impl TotalInterchainTransferUsersStatement {
    /// Split out from `get_statement_with_context` so tests can render it with an
    /// explicit filter and no `UpdateContext` (hence no database connections).
    fn build(filter: &InterchainFilter) -> Statement {
        use crosschain_transfers::Column as C;
        let arm = |column: C| {
            filter
                .transfers_query()
                .select_only()
                .expr_as(column.into_expr(), "addr")
                .filter(column.is_not_null())
                .into_query()
        };
        let mut addresses = arm(C::SenderAddress);
        addresses.union(UnionType::Distinct, arm(C::RecipientAddress));
        let counted = Query::select()
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .from_subquery(addresses, Alias::new("u"))
            .to_owned();
        DbBackend::Postgres.build(&counted)
    }
}

impl StatementFromUpdateTime for TotalInterchainTransferUsersStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> Statement {
        Self::build(&cx.interchain_filter)
    }
}

impl InterchainFiltered for TotalInterchainTransferUsersStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Transfers;
    /// One per `UNION` arm.
    const EXPECTED_APPLICATIONS: usize = 2;
    const CHART_NAME: &'static str = "totalInterchainTransferUsers";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter)
    }
}

pub type TotalInterchainTransferUsersRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalInterchainTransferUsersStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInterchainTransferUsers".into()
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

pub type TotalInterchainTransferUsers =
    DirectPointLocalDbChartSource<MapToString<TotalInterchainTransferUsersRemote>, Properties>;

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
            TotalInterchainTransferUsersStatement::build(&test_interchain_home_chain_filter(1));
        // the filter appears once per arm — that is what `EXPECTED_APPLICATIONS`
        // declares, and what the coverage test counts
        let expected = r#"
            SELECT COUNT(*) AS "value" FROM (SELECT
                "crosschain_transfers"."sender_address" AS "addr"
                FROM "crosschain_transfers"
                WHERE ("crosschain_transfers"."token_src_chain_id" = 1
                       OR "crosschain_transfers"."token_dst_chain_id" = 1)
                  AND "crosschain_transfers"."sender_address" IS NOT NULL
                UNION (SELECT
                "crosschain_transfers"."recipient_address" AS "addr"
                FROM "crosschain_transfers"
                WHERE ("crosschain_transfers"."token_src_chain_id" = 1
                       OR "crosschain_transfers"."token_dst_chain_id" = 1)
                  AND "crosschain_transfers"."recipient_address" IS NOT NULL)) AS "u"
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()))
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interchain_transfer_users() {
        simple_test_counter_interchain::<TotalInterchainTransferUsers>(
            "update_total_interchain_transfer_users",
            "8",
            None,
            InterchainFilter::default(),
        )
        .await;
    }

    /// The first filtered coverage this counter has ever had. The fixture cycles
    /// eight distinct addresses across all transfers, so a filter is visible here
    /// only when it narrows the transfer set enough to drop some of them —
    /// bridge 2's three transfers use six.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_interchain_transfer_users_filtered() {
        simple_test_counter_interchain::<TotalInterchainTransferUsers>(
            "transfer_users_bridge_2",
            "6",
            None,
            test_interchain_filter(ChainBridgeFilter {
                bridge_ids: Some(vec![MOCK_SECOND_BRIDGE_ID]),
                ..Default::default()
            }),
        )
        .await;

        simple_test_counter_interchain::<TotalInterchainTransferUsers>(
            "transfer_users_cp_2_3",
            "7",
            None,
            test_interchain_filter(ChainBridgeFilter {
                counterparty_chain_ids: Some(vec![2, 3]),
                ..Default::default()
            }),
        )
        .await;

        simple_test_counter_interchain::<TotalInterchainTransferUsers>(
            "transfer_users_home_1",
            "8",
            None,
            test_interchain_home_chain_filter(1),
        )
        .await;

        simple_test_counter_interchain::<TotalInterchainTransferUsers>(
            "transfer_users_horizon",
            "8",
            None,
            test_interchain_filter_with_horizon(
                ChainBridgeFilter::default(),
                Some(mock_interchain_horizon()),
            ),
        )
        .await;
    }
}
