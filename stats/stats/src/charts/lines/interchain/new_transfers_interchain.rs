// SPDX-License-Identifier: LicenseRef-Blockscout

//! New interchain transfers per day, **within the configured interchain slice**.
//!
//! Counts `crosschain_transfers` rows admitted by the shared read filter, which
//! is evaluated on the transfer's own `token_src_chain_id` /
//! `token_dst_chain_id` / `bridge_id`.
//!
//! The join to `crosschain_messages` exists **only** to reach the parent
//! message's `init_timestamp`: a transfer has no timestamp of its own, so that
//! is the time axis for every transfer chart. The join is composite
//! (`(message_id, bridge_id)`) via the declared SeaORM relation — a
//! `message_id`-only join fans transfers out across bridges that reuse a numeric
//! message id.

use std::ops::Range;

use interchain_indexer_entity::crosschain_messages;

use crate::{
    chart_prelude::*,
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterTarget, InterchainFiltered,
    },
};

pub struct NewTransfersInterchainStatement;
impl_db_choice!(NewTransfersInterchainStatement, UsePrimaryDB);

impl NewTransfersInterchainStatement {
    /// Split out from `get_statement_with_context` so tests can render it with an
    /// explicit filter and no `UpdateContext` (hence no database connections).
    fn build(filter: &InterchainFilter, range: Option<Range<DateTime<Utc>>>) -> Statement {
        const DATE: &str = "date";
        let time_axis = crosschain_messages::Column::InitTimestamp;
        let query = filter
            .transfers_joined_query()
            .select_only()
            .expr_as(time_axis.into_expr().cast_as(DATE), DATE)
            .expr_as(
                Func::count(Asterisk.into_column_ref()).cast_as("TEXT"),
                "value",
            );
        let query = match &range {
            Some(range) => datetime_range_filter(query, time_axis, range),
            None => query,
        };
        query
            .group_by(Expr::col(Alias::new(DATE)))
            .build(DbBackend::Postgres)
    }
}

impl StatementFromRange for NewTransfersInterchainStatement {
    fn get_statement_with_context(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTime<Utc>>>,
    ) -> Statement {
        Self::build(&cx.interchain_filter, range)
    }
}

impl InterchainFiltered for NewTransfersInterchainStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Transfers;
    const CHART_NAME: &'static str = "newTransfersInterchain";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter, None)
    }
}

pub type NewTransfersInterchainRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        NewTransfersInterchainStatement,
        NaiveDate,
        String,
        QueryFullIndexerTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newTransfersInterchain".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
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

pub type NewTransfersInterchain =
    DirectVecLocalDbChartSource<NewTransfersInterchainRemote, Batch30Days, Properties>;
pub type NewTransfersInterchainInt = MapParseTo<StripExt<NewTransfersInterchain>, i64>;
pub type NewTransfersInterchainWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTransfersInterchainInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewTransfersInterchainMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTransfersInterchainInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewTransfersInterchainMonthlyInt =
    MapParseTo<StripExt<NewTransfersInterchainMonthly>, i64>;
pub type NewTransfersInterchainYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTransfersInterchainMonthlyInt, Year>>,
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
            MOCK_SECOND_BRIDGE_ID, test_interchain_filter, test_interchain_home_chain_filter,
        },
        normalize_sql,
        point_construction::dt,
        simple_test::simple_test_chart_interchain,
    };

    #[test]
    fn statement_is_correct() {
        let actual = NewTransfersInterchainStatement::build(
            &test_interchain_home_chain_filter(1),
            Some(dt("2023-01-01T00:00:00").and_utc()..dt("2023-01-02T00:00:00").and_utc()),
        );
        let expected = r#"
            SELECT
                CAST("crosschain_messages"."init_timestamp" AS date) AS "date",
                CAST(COUNT(*) AS TEXT) AS "value"
            FROM "crosschain_transfers"
            INNER JOIN "crosschain_messages"
                ON "crosschain_transfers"."message_id" = "crosschain_messages"."id"
               AND "crosschain_transfers"."bridge_id" = "crosschain_messages"."bridge_id"
            WHERE ("crosschain_transfers"."token_src_chain_id" = 1
                   OR "crosschain_transfers"."token_dst_chain_id" = 1)
              AND "crosschain_messages"."init_timestamp" < '2023-01-02 00:00:00.000000 +00:00'
              AND "crosschain_messages"."init_timestamp" >= '2023-01-01 00:00:00.000000 +00:00'
            GROUP BY "date"
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()))
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_interchain() {
        simple_test_chart_interchain::<NewTransfersInterchain>(
            "update_new_transfers_interchain",
            vec![
                ("2022-12-20", "2"),
                ("2022-12-21", "3"),
                ("2022-12-23", "1"),
                ("2022-12-26", "5"),
                ("2022-12-27", "5"),
                ("2023-01-01", "3"),
                ("2023-01-04", "3"),
                ("2023-01-10", "6"),
                ("2023-01-11", "1"),
                ("2023-01-20", "1"),
                ("2023-01-21", "3"),
                ("2023-02-01", "7"),
                ("2023-02-05", "1"),
                ("2023-02-06", "3"),
                ("2023-02-07", "1"),
                ("2023-02-08", "1"),
                ("2023-02-09", "1"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    /// Redefined in place: this chart used to ignore filtering entirely.
    ///
    /// The bridge-2 case is also the composite-join regression test on the line
    /// side: bridge 2's message id `1` collides with bridge 1's, so a
    /// `message_id`-only join would attribute bridge-1's two transfers on message
    /// 1 to this day as well.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn new_transfers_interchain_filtered() {
        simple_test_chart_interchain::<NewTransfersInterchain>(
            "new_transfers_interchain_bridge_2",
            vec![("2023-02-06", "3")],
            test_interchain_filter(ChainBridgeFilter {
                bridge_ids: Some(vec![MOCK_SECOND_BRIDGE_ID]),
                ..Default::default()
            }),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_interchain_weekly() {
        simple_test_chart_interchain::<NewTransfersInterchainWeekly>(
            "update_new_transfers_interchain_weekly",
            vec![
                ("2022-12-19", "6"),
                ("2022-12-26", "13"),
                ("2023-01-02", "3"),
                ("2023-01-09", "7"),
                ("2023-01-16", "4"),
                ("2023-01-30", "8"),
                ("2023-02-06", "6"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_interchain_monthly() {
        simple_test_chart_interchain::<NewTransfersInterchainMonthly>(
            "update_new_transfers_interchain_monthly",
            vec![
                ("2022-12-01", "16"),
                ("2023-01-01", "17"),
                ("2023-02-01", "14"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_interchain_yearly() {
        simple_test_chart_interchain::<NewTransfersInterchainYearly>(
            "update_new_transfers_interchain_yearly",
            vec![("2022-01-01", "16"), ("2023-01-01", "31")],
            InterchainFilter::default(),
        )
        .await;
    }
}
