//! New interchain messages line chart (total indexed per day, no filters).
//! Counts messages by init_timestamp date from crosschain_messages. Does not use interchain_primary_id.

use std::ops::Range;

use crate::{
    chart_prelude::*,
    charts::db_interaction::read::QueryFullIndexerTimestampRange,
    data_source::kinds::{
        data_manipulation::{
            map::{MapParseTo, MapToString, StripExt},
            resolutions::sum::SumLowerResolution,
        },
        local_db::parameters::update::batching::parameters::{
            Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
        },
    },
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
};
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::Statement;

pub struct NewMessagesInterchainStatement;

impl StatementFromRange for NewMessagesInterchainStatement {
    fn get_statement_with_context(
        _cx: &UpdateContext<'_>,
        range: Option<Range<DateTime<Utc>>>,
    ) -> Statement {
        sql_with_range_filter_opt!(
            sea_orm::DbBackend::Postgres,
            r#"
                SELECT
                    init_timestamp::date AS date,
                    COUNT(*)::TEXT AS value
                FROM crosschain_messages
                WHERE true
                    {filter}
                GROUP BY init_timestamp::date
            "#,
            [],
            "init_timestamp::timestamp",
            range,
        )
    }
}

impl_db_choice!(NewMessagesInterchainStatement, UsePrimaryDB);

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
    use super::*;
    use crate::tests::simple_test::simple_test_chart_interchain;

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
                ("2023-02-10", "1"),
            ],
            None,
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
                ("2023-02-06", "1"),
            ],
            None,
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
                ("2023-02-01", "4"),
            ],
            None,
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_interchain_yearly() {
        simple_test_chart_interchain::<NewMessagesInterchainYearly>(
            "update_new_messages_interchain_yearly",
            vec![("2022-01-01", "7"), ("2023-01-01", "14")],
            None,
        )
        .await;
    }
}
