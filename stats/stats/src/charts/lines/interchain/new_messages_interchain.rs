// SPDX-License-Identifier: LicenseRef-Blockscout

//! New interchain messages per day, **within the configured interchain slice**.
//!
//! Counts `crosschain_messages` rows admitted by the shared read filter
//! (`STATS__INTERCHAIN_FILTER__*` plus the observability horizon), grouped by
//! `init_timestamp` date. The chart adds no term of its own beyond the time
//! axis.

use std::ops::Range;

use interchain_indexer_entity::crosschain_messages;

use crate::{
    chart_prelude::*,
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterTarget, InterchainFiltered,
    },
};

pub struct NewMessagesInterchainStatement;
impl_db_choice!(NewMessagesInterchainStatement, UsePrimaryDB);

impl NewMessagesInterchainStatement {
    /// Split out from `get_statement_with_context` so tests can render it with an
    /// explicit filter and no `UpdateContext` (hence no database connections).
    fn build(filter: &InterchainFilter, range: Option<Range<DateTime<Utc>>>) -> Statement {
        use crosschain_messages::Column as C;
        const DATE: &str = "date";
        let query = filter
            .messages_query()
            .select_only()
            .expr_as(C::InitTimestamp.into_expr().cast_as(DATE), DATE)
            // `PullAllWithAndSort<_, _, String, _>` reads `value` as text
            .expr_as(
                Func::count(Asterisk.into_column_ref()).cast_as("TEXT"),
                "value",
            );
        let query = match &range {
            Some(range) => datetime_range_filter(query, C::InitTimestamp, range),
            None => query,
        };
        query
            .group_by(Expr::col(Alias::new(DATE)))
            .build(DbBackend::Postgres)
    }
}

impl StatementFromRange for NewMessagesInterchainStatement {
    fn get_statement_with_context(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTime<Utc>>>,
    ) -> Statement {
        Self::build(&cx.interchain_filter, range)
    }
}

impl InterchainFiltered for NewMessagesInterchainStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Messages;
    const CHART_NAME: &'static str = "newMessagesInterchain";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter, None)
    }
}

pub type NewMessagesInterchainRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        NewMessagesInterchainStatement,
        NaiveDate,
        String,
        QueryFullIndexerTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newMessagesInterchain".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus::LEAST_RESTRICTIVE.with_interchain(InterchainIndexingStatus::CaughtUp)
    }
}

define_and_impl_resolution_properties!(
    define_and_impl: {
        WeeklyProperties: Week,
        MonthlyProperties: Month,
        YearlyProperties: Year,
    },
    base_impl: Properties
);

pub type NewMessagesInterchain =
    DirectVecLocalDbChartSource<NewMessagesInterchainRemote, Batch30Days, Properties>;
pub type NewMessagesInterchainInt = MapParseTo<StripExt<NewMessagesInterchain>, i64>;
pub type NewMessagesInterchainWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewMessagesInterchainInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewMessagesInterchainMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewMessagesInterchainInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewMessagesInterchainMonthlyInt = MapParseTo<StripExt<NewMessagesInterchainMonthly>, i64>;
pub type NewMessagesInterchainYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewMessagesInterchainMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

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
        simple_test::simple_test_chart_interchain,
    };

    #[test]
    fn statement_is_correct() {
        let actual = NewMessagesInterchainStatement::build(
            &test_interchain_home_chain_filter(1),
            Some(dt("2023-01-01T00:00:00").and_utc()..dt("2023-01-02T00:00:00").and_utc()),
        );
        let expected = r#"
            SELECT
                CAST("crosschain_messages"."init_timestamp" AS date) AS "date",
                CAST(COUNT(*) AS TEXT) AS "value"
            FROM "crosschain_messages"
            WHERE ("crosschain_messages"."src_chain_id" = 1
                   OR "crosschain_messages"."dst_chain_id" = 1)
              AND "crosschain_messages"."init_timestamp" < '2023-01-02 00:00:00.000000 +00:00'
              AND "crosschain_messages"."init_timestamp" >= '2023-01-01 00:00:00.000000 +00:00'
            GROUP BY "date"
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()))
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_interchain() {
        simple_test_chart_interchain::<NewMessagesInterchain>(
            "update_new_messages_interchain",
            vec![
                ("2022-12-20", "1"),
                ("2022-12-21", "2"),
                ("2022-12-23", "1"),
                ("2022-12-26", "1"),
                ("2022-12-27", "2"),
                ("2023-01-01", "2"),
                ("2023-01-02", "1"),
                ("2023-01-04", "1"),
                ("2023-01-10", "2"),
                ("2023-01-11", "1"),
                ("2023-01-20", "1"),
                ("2023-01-21", "2"),
                ("2023-02-01", "2"),
                ("2023-02-05", "1"),
                ("2023-02-06", "2"),
                ("2023-02-07", "1"),
                ("2023-02-08", "1"),
                ("2023-02-09", "1"),
                ("2023-02-10", "1"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    /// Redefined in place: this chart used to ignore filtering entirely.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn new_messages_interchain_filtered() {
        simple_test_chart_interchain::<NewMessagesInterchain>(
            "new_messages_interchain_home_1",
            vec![
                ("2022-12-20", "1"),
                ("2022-12-21", "2"),
                ("2022-12-23", "1"),
                ("2022-12-26", "1"),
                ("2022-12-27", "1"),
                ("2023-01-01", "2"),
                ("2023-01-02", "1"),
                ("2023-01-04", "1"),
                ("2023-01-10", "2"),
                ("2023-01-20", "1"),
                ("2023-01-21", "2"),
                ("2023-02-01", "2"),
                ("2023-02-05", "1"),
                ("2023-02-06", "1"),
                ("2023-02-07", "1"),
                ("2023-02-08", "1"),
                ("2023-02-09", "1"),
                ("2023-02-10", "1"),
            ],
            test_interchain_home_chain_filter(1),
        )
        .await;

        simple_test_chart_interchain::<NewMessagesInterchain>(
            "new_messages_interchain_bridge_2",
            vec![("2023-02-06", "2")],
            test_interchain_filter(ChainBridgeFilter {
                bridge_ids: Some(vec![MOCK_SECOND_BRIDGE_ID]),
                ..Default::default()
            }),
        )
        .await;
    }

    /// The horizon drops message 22 (NULL destination), message 23 (`dst = 4`)
    /// and both bridge-2 messages, leaving 2023-02-09 as the only new date.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn new_messages_interchain_horizon() {
        simple_test_chart_interchain::<NewMessagesInterchain>(
            "new_messages_interchain_horizon",
            vec![
                ("2022-12-20", "1"),
                ("2022-12-21", "2"),
                ("2022-12-23", "1"),
                ("2022-12-26", "1"),
                ("2022-12-27", "2"),
                ("2023-01-01", "2"),
                ("2023-01-02", "1"),
                ("2023-01-04", "1"),
                ("2023-01-10", "2"),
                ("2023-01-11", "1"),
                ("2023-01-20", "1"),
                ("2023-01-21", "2"),
                ("2023-02-01", "2"),
                ("2023-02-05", "1"),
                ("2023-02-09", "1"),
                ("2023-02-10", "1"),
            ],
            test_interchain_filter_with_horizon(
                ChainBridgeFilter::default(),
                Some(mock_interchain_horizon()),
            ),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_interchain_weekly() {
        simple_test_chart_interchain::<NewMessagesInterchainWeekly>(
            "update_new_messages_interchain_weekly",
            vec![
                ("2022-12-19", "4"),
                ("2022-12-26", "5"),
                ("2023-01-02", "2"),
                ("2023-01-09", "3"),
                ("2023-01-16", "3"),
                ("2023-01-30", "3"),
                ("2023-02-06", "6"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_interchain_monthly() {
        simple_test_chart_interchain::<NewMessagesInterchainMonthly>(
            "update_new_messages_interchain_monthly",
            vec![
                ("2022-12-01", "7"),
                ("2023-01-01", "10"),
                ("2023-02-01", "9"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_interchain_yearly() {
        simple_test_chart_interchain::<NewMessagesInterchainYearly>(
            "update_new_messages_interchain_yearly",
            vec![("2022-01-01", "7"), ("2023-01-01", "19")],
            InterchainFilter::default(),
        )
        .await;
    }
}
