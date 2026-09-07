// SPDX-License-Identifier: LicenseRef-Blockscout

//! New interchain messages in the last 24 hours, **within the configured
//! interchain slice**.
//!
//! Counts `crosschain_messages` rows admitted by the shared read filter
//! (`STATS__INTERCHAIN_FILTER__*` plus the observability horizon) whose
//! `init_timestamp` falls in the rolling 24-hour window ending at the update
//! time. The chart adds no term of its own beyond that window — it is the
//! 24-hour slice of `newMessagesInterchain`, not an observability statement, so
//! neither `src_tx_hash` nor `dst_tx_hash` is consulted.

use std::ops::Range;

use interchain_indexer_entity::crosschain_messages;

use crate::{
    chart_prelude::*,
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterTarget, InterchainFiltered,
    },
    range::inclusive_range_to_exclusive,
};

pub struct NewMessagesInterchain24hStatement;
impl_db_choice!(NewMessagesInterchain24hStatement, UsePrimaryDB);

impl NewMessagesInterchain24hStatement {
    /// Split out from `get_statement_with_context` so tests can render it with an
    /// explicit filter and no `UpdateContext` (hence no database connections).
    ///
    /// `range` is `Option` only so that [`InterchainFiltered::render`] can pass
    /// `None`, exactly as the interchain line charts do — the coverage test
    /// counts filter-predicate renderings and needs no window.
    /// `get_statement_with_context` is the only production caller and always
    /// passes `Some`.
    fn build(filter: &InterchainFilter, range: Option<Range<DateTime<Utc>>>) -> Statement {
        use crosschain_messages::Column as C;
        let query = filter
            .messages_query()
            .select_only()
            // `PullOneNowValue<_, _, i64>` reads a column named `value`, and
            // Postgres `COUNT(*)` is already `bigint` — no cast.
            .expr_as(Func::count(Asterisk.into_column_ref()), "value");
        let query = match &range {
            Some(range) => datetime_range_filter(query, C::InitTimestamp, range),
            None => query,
        };
        query.build(DbBackend::Postgres)
    }
}

impl StatementFromUpdateTime for NewMessagesInterchain24hStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> Statement {
        Self::build(
            &cx.interchain_filter,
            Some(inclusive_range_to_exclusive(interval_24h(cx.time))),
        )
    }
}

impl InterchainFiltered for NewMessagesInterchain24hStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Messages;
    const CHART_NAME: &'static str = "newMessagesInterchain24h";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter, None)
    }
}

pub type NewMessagesInterchain24hRemote =
    RemoteDatabaseSource<PullOneNowValue<NewMessagesInterchain24hStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newMessagesInterchain24h".into()
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

pub type NewMessagesInterchain24h =
    DirectPointLocalDbChartSource<MapToString<NewMessagesInterchain24hRemote>, Properties>;

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
        let actual = NewMessagesInterchain24hStatement::build(
            &test_interchain_home_chain_filter(1),
            Some(dt("2023-01-01T00:00:00").and_utc()..dt("2023-01-02T00:00:00").and_utc()),
        );
        let expected = r#"
            SELECT COUNT(*) AS "value"
            FROM "crosschain_messages"
            WHERE ("crosschain_messages"."src_chain_id" = 1
                   OR "crosschain_messages"."dst_chain_id" = 1)
              AND "crosschain_messages"."init_timestamp" < '2023-01-02 00:00:00.000000 +00:00'
              AND "crosschain_messages"."init_timestamp" >= '2023-01-01 00:00:00.000000 +00:00'
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()))
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_interchain_24h() {
        simple_test_counter_interchain::<NewMessagesInterchain24h>(
            "update_new_messages_interchain_24h",
            "2",
            Some(dt("2023-01-11T00:00:00")),
            InterchainFilter::default(),
        )
        .await;
    }

    /// Catches a transfers-shaped mistake if ever copy-pasted onto messages: this
    /// pins the messages side of the same window, filtered on the message's own
    /// route (`home_chain_id = 3`), so only message 12 (route `1 → 3`) qualifies.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn new_messages_interchain_24h_home_3() {
        simple_test_counter_interchain::<NewMessagesInterchain24h>(
            "new_messages_interchain_24h_home_3",
            "1",
            Some(dt("2023-01-11T00:00:00")),
            test_interchain_home_chain_filter(3),
        )
        .await;
    }

    /// Unfiltered baseline for [`new_messages_interchain_24h_horizon`]: the
    /// window holds exactly message 24, so the horizon case's `1` means "the
    /// message survived the horizon", not "the window happened to be empty".
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn new_messages_interchain_24h_horizon_baseline() {
        simple_test_counter_interchain::<NewMessagesInterchain24h>(
            "new_messages_interchain_24h_horizon_baseline",
            "1",
            Some(dt("2023-02-09T11:00:00")),
            InterchainFilter::default(),
        )
        .await;
    }

    /// The horizon reaches the new counter: message 24's route `1→2` is inside
    /// bridge 1's observed chain set, so it survives — same count as
    /// [`new_messages_interchain_24h_horizon_baseline`], while the same window's
    /// transfer is dropped (see `new_transfers_interchain_24h_horizon`).
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn new_messages_interchain_24h_horizon() {
        simple_test_counter_interchain::<NewMessagesInterchain24h>(
            "new_messages_interchain_24h_horizon",
            "1",
            Some(dt("2023-02-09T11:00:00")),
            test_interchain_filter_with_horizon(
                ChainBridgeFilter::default(),
                Some(mock_interchain_horizon()),
            ),
        )
        .await;
    }

    /// Unfiltered companion of [`new_messages_interchain_24h_bridge_2`]: every
    /// message in this window belongs to bridge 2 (`id = 1` at 10:00, `id = 100`
    /// at 11:00), so the bridge dimension drops nothing here and both cases read
    /// `2`. Pins that, and keeps this side symmetric with
    /// `new_transfers_interchain_24h_id_collision_window`, where the same window
    /// is what catches a `message_id`-only join.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn new_messages_interchain_24h_id_collision_window() {
        simple_test_counter_interchain::<NewMessagesInterchain24h>(
            "new_messages_interchain_24h_id_collision_window",
            "2",
            Some(dt("2023-02-06T12:00:00")),
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn new_messages_interchain_24h_bridge_2() {
        simple_test_counter_interchain::<NewMessagesInterchain24h>(
            "new_messages_interchain_24h_bridge_2",
            "2",
            Some(dt("2023-02-06T12:00:00")),
            test_interchain_filter(ChainBridgeFilter {
                bridge_ids: Some(vec![MOCK_SECOND_BRIDGE_ID]),
                ..Default::default()
            }),
        )
        .await;
    }
}
