// SPDX-License-Identifier: LicenseRef-Blockscout

//! New interchain messages sent per day, within the configured interchain slice.
//!
//! Counts messages admitted by the shared read filter whose source event was
//! indexed (`src_tx_hash IS NOT NULL`), grouped by `init_timestamp` date. When
//! `STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID` is set, "sent" additionally means
//! `src_chain_id = home`; with no home chain configured the chart degrades to
//! "source-side observed", which the server warns about at startup.

use std::ops::Range;

use interchain_indexer_entity::crosschain_messages;

use crate::{
    chart_prelude::*,
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterTarget, InterchainFiltered,
    },
};

pub struct NewMessagesSentInterchainStatement;
impl_db_choice!(NewMessagesSentInterchainStatement, UsePrimaryDB);

impl NewMessagesSentInterchainStatement {
    /// Split out from `get_statement_with_context` so tests can render it with an
    /// explicit filter and no `UpdateContext` (hence no database connections).
    fn build(filter: &InterchainFilter, range: Option<Range<DateTime<Utc>>>) -> Statement {
        use crosschain_messages::Column as C;
        const DATE: &str = "date";
        let query = filter
            .messages_query()
            .select_only()
            .expr_as(C::InitTimestamp.into_expr().cast_as(DATE), DATE)
            .expr_as(
                Func::count(Asterisk.into_column_ref()).cast_as("TEXT"),
                "value",
            )
            .filter(C::SrcTxHash.is_not_null())
            .apply_if(filter.home_chain_id(), |query, home| {
                query.filter(C::SrcChainId.eq(home))
            });
        let query = match &range {
            Some(range) => datetime_range_filter(query, C::InitTimestamp, range),
            None => query,
        };
        query
            .group_by(Expr::col(Alias::new(DATE)))
            .build(DbBackend::Postgres)
    }
}

impl StatementFromRange for NewMessagesSentInterchainStatement {
    fn get_statement_with_context(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTime<Utc>>>,
    ) -> Statement {
        Self::build(&cx.interchain_filter, range)
    }
}

impl InterchainFiltered for NewMessagesSentInterchainStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Messages;
    const CHART_NAME: &'static str = "newMessagesSentInterchain";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter, None)
    }
}

pub type NewMessagesSentInterchainRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        NewMessagesSentInterchainStatement,
        NaiveDate,
        String,
        QueryFullIndexerTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newMessagesSentInterchain".into()
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

pub type NewMessagesSentInterchain =
    DirectVecLocalDbChartSource<NewMessagesSentInterchainRemote, Batch30Days, Properties>;
pub type NewMessagesSentInterchainInt = MapParseTo<StripExt<NewMessagesSentInterchain>, i64>;
pub type NewMessagesSentInterchainWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewMessagesSentInterchainInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewMessagesSentInterchainMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewMessagesSentInterchainInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewMessagesSentInterchainMonthlyInt =
    MapParseTo<StripExt<NewMessagesSentInterchainMonthly>, i64>;
pub type NewMessagesSentInterchainYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewMessagesSentInterchainMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{
        mock_interchain::test_interchain_home_chain_filter,
        simple_test::simple_test_chart_interchain,
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_sent_interchain() {
        simple_test_chart_interchain::<NewMessagesSentInterchain>(
            "update_new_messages_sent_interchain",
            vec![
                ("2022-12-20", "1"),
                ("2022-12-21", "2"),
                ("2022-12-26", "1"),
                ("2022-12-27", "1"),
                ("2023-01-01", "2"),
                ("2023-01-04", "1"),
                ("2023-01-10", "2"),
                ("2023-01-20", "1"),
                ("2023-01-21", "1"),
                ("2023-02-01", "2"),
                ("2023-02-06", "2"),
                ("2023-02-07", "1"),
                ("2023-02-08", "1"),
                ("2023-02-09", "1"),
                ("2023-02-10", "1"),
            ],
            InterchainFilter::default(),
        )
        .await;

        simple_test_chart_interchain::<NewMessagesSentInterchain>(
            "update_new_messages_sent_interchain_primary_1",
            vec![
                ("2022-12-20", "1"),
                ("2022-12-21", "1"),
                ("2022-12-26", "1"),
                ("2023-01-01", "2"),
                ("2023-01-04", "1"),
                ("2023-01-10", "1"),
                ("2023-01-20", "1"),
                ("2023-02-01", "2"),
                ("2023-02-06", "1"),
                ("2023-02-07", "1"),
                ("2023-02-08", "1"),
                ("2023-02-09", "1"),
                ("2023-02-10", "1"),
            ],
            test_interchain_home_chain_filter(1),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_sent_interchain_weekly() {
        simple_test_chart_interchain::<NewMessagesSentInterchainWeekly>(
            "update_new_messages_sent_interchain_weekly",
            vec![
                ("2022-12-19", "3"),
                ("2022-12-26", "4"),
                ("2023-01-02", "1"),
                ("2023-01-09", "2"),
                ("2023-01-16", "2"),
                ("2023-01-30", "2"),
                ("2023-02-06", "6"),
            ],
            InterchainFilter::default(),
        )
        .await;

        simple_test_chart_interchain::<NewMessagesSentInterchainWeekly>(
            "update_new_messages_sent_interchain_weekly_primary_1",
            vec![
                ("2022-12-19", "2"),
                ("2022-12-26", "3"),
                ("2023-01-02", "1"),
                ("2023-01-09", "1"),
                ("2023-01-16", "1"),
                ("2023-01-30", "2"),
                ("2023-02-06", "5"),
            ],
            test_interchain_home_chain_filter(1),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_sent_interchain_monthly() {
        simple_test_chart_interchain::<NewMessagesSentInterchainMonthly>(
            "update_new_messages_sent_interchain_monthly",
            vec![
                ("2022-12-01", "5"),
                ("2023-01-01", "7"),
                ("2023-02-01", "8"),
            ],
            InterchainFilter::default(),
        )
        .await;

        simple_test_chart_interchain::<NewMessagesSentInterchainMonthly>(
            "update_new_messages_sent_interchain_monthly_primary_1",
            vec![
                ("2022-12-01", "3"),
                ("2023-01-01", "5"),
                ("2023-02-01", "7"),
            ],
            test_interchain_home_chain_filter(1),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_sent_interchain_yearly() {
        simple_test_chart_interchain::<NewMessagesSentInterchainYearly>(
            "update_new_messages_sent_interchain_yearly",
            vec![("2022-01-01", "5"), ("2023-01-01", "15")],
            InterchainFilter::default(),
        )
        .await;

        simple_test_chart_interchain::<NewMessagesSentInterchainYearly>(
            "update_new_messages_sent_interchain_yearly_primary_1",
            vec![("2022-01-01", "3"), ("2023-01-01", "12")],
            test_interchain_home_chain_filter(1),
        )
        .await;
    }
}
