// SPDX-License-Identifier: LicenseRef-Blockscout

//! New interchain transfers in the last 24 hours, **within the configured
//! interchain slice**.
//!
//! Counts `crosschain_transfers` rows admitted by the shared read filter, which
//! is evaluated on the transfer's own `token_src_chain_id` /
//! `token_dst_chain_id` / `bridge_id` — never on the joined message's route. The
//! composite join to `crosschain_messages` exists **only** to reach
//! `init_timestamp`, since a transfer has no timestamp of its own; never
//! hand-write that join. The chart is the 24-hour slice of
//! `newTransfersInterchain`, not an observability statement, so neither
//! `src_tx_hash` nor `dst_tx_hash` is consulted.

use std::ops::Range;

use interchain_indexer_entity::crosschain_messages;

use crate::{
    chart_prelude::*,
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterTarget, InterchainFiltered,
    },
    range::inclusive_range_to_exclusive,
};

pub struct NewTransfersInterchain24hStatement;
impl_db_choice!(NewTransfersInterchain24hStatement, UsePrimaryDB);

impl NewTransfersInterchain24hStatement {
    /// Split out from `get_statement_with_context` so tests can render it with an
    /// explicit filter and no `UpdateContext` (hence no database connections).
    ///
    /// `range` is `Option` only so that [`InterchainFiltered::render`] can pass
    /// `None`, exactly as the interchain line charts do — the coverage test
    /// counts filter-predicate renderings and needs no window.
    /// `get_statement_with_context` is the only production caller and always
    /// passes `Some`.
    fn build(filter: &InterchainFilter, range: Option<Range<DateTime<Utc>>>) -> Statement {
        let time_axis = crosschain_messages::Column::InitTimestamp;
        let query = filter
            .transfers_joined_query()
            .select_only()
            // `PullOneNowValue<_, _, i64>` reads a column named `value`, and
            // Postgres `COUNT(*)` is already `bigint` — no cast.
            .expr_as(Func::count(Asterisk.into_column_ref()), "value");
        let query = match &range {
            Some(range) => datetime_range_filter(query, time_axis, range),
            None => query,
        };
        query.build(DbBackend::Postgres)
    }
}

impl StatementFromUpdateTime for NewTransfersInterchain24hStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> Statement {
        Self::build(
            &cx.interchain_filter,
            Some(inclusive_range_to_exclusive(interval_24h(cx.time))),
        )
    }
}

impl InterchainFiltered for NewTransfersInterchain24hStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Transfers;
    const CHART_NAME: &'static str = "newTransfersInterchain24h";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter, None)
    }
}

pub type NewTransfersInterchain24hRemote =
    RemoteDatabaseSource<PullOneNowValue<NewTransfersInterchain24hStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newTransfersInterchain24h".into()
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

pub type NewTransfersInterchain24h =
    DirectPointLocalDbChartSource<MapToString<NewTransfersInterchain24hRemote>, Properties>;

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
        point_construction::dt,
        simple_test::simple_test_counter_interchain,
    };

    #[test]
    fn statement_is_correct() {
        let actual = NewTransfersInterchain24hStatement::build(
            &test_interchain_home_chain_filter(1),
            Some(dt("2023-01-01T00:00:00").and_utc()..dt("2023-01-02T00:00:00").and_utc()),
        );
        let expected = r#"
            SELECT COUNT(*) AS "value"
            FROM "crosschain_transfers"
            INNER JOIN "crosschain_messages"
                ON "crosschain_transfers"."message_id" = "crosschain_messages"."id"
               AND "crosschain_transfers"."bridge_id" = "crosschain_messages"."bridge_id"
            WHERE ("crosschain_transfers"."token_src_chain_id" = 1
                   OR "crosschain_transfers"."token_dst_chain_id" = 1)
              AND "crosschain_messages"."init_timestamp" < '2023-01-02 00:00:00.000000 +00:00'
              AND "crosschain_messages"."init_timestamp" >= '2023-01-01 00:00:00.000000 +00:00'
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()))
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_interchain_24h() {
        simple_test_counter_interchain::<NewTransfersInterchain24h>(
            "update_new_transfers_interchain_24h",
            "6",
            Some(dt("2023-01-11T00:00:00")),
            InterchainFilter::default(),
        )
        .await;
    }

    /// The case that fails if the transfers statement filters on the joined
    /// message's route instead of the transfer's own token chains: on
    /// 2023-01-10 the window holds message 12 (route `1→3`, two transfers with
    /// token chains `(1,3)`) and message 13 (route `2→1`, four transfers with
    /// `(2,1)`). With `home_chain_id = 3` only message 12 and its two transfers
    /// qualify.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn new_transfers_interchain_24h_home_3() {
        simple_test_counter_interchain::<NewTransfersInterchain24h>(
            "new_transfers_interchain_24h_home_3",
            "2",
            Some(dt("2023-01-11T00:00:00")),
            test_interchain_home_chain_filter(3),
        )
        .await;
    }

    /// Proves the observability horizon reaches the new counters: message 24's
    /// route `1→2` is inside bridge 1's observed chain set so the message
    /// survives, while its transfer's token chains `3→4` are not, so the
    /// transfer is dropped.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn new_transfers_interchain_24h_horizon() {
        simple_test_counter_interchain::<NewTransfersInterchain24h>(
            "new_transfers_interchain_24h_horizon",
            "0",
            Some(dt("2023-02-09T11:00:00")),
            test_interchain_filter_with_horizon(
                ChainBridgeFilter::default(),
                Some(mock_interchain_horizon()),
            ),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn new_transfers_interchain_24h_bridge_2() {
        simple_test_counter_interchain::<NewTransfersInterchain24h>(
            "new_transfers_interchain_24h_bridge_2",
            "3",
            Some(dt("2023-02-06T12:00:00")),
            test_interchain_filter(ChainBridgeFilter {
                bridge_ids: Some(vec![MOCK_SECOND_BRIDGE_ID]),
                ..Default::default()
            }),
        )
        .await;
    }
}
