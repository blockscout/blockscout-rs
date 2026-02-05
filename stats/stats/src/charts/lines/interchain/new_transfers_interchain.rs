//! New interchain transfers line chart (total indexed per day, no filters).
//! Counts transfers by associated message's init_timestamp date. Does not use interchain_primary_id.

use std::ops::Range;

use crate::{
    chart_prelude::*,
    charts::db_interaction::read::QueryFullIndexerTimestampRange,
    data_source::kinds::data_manipulation::{
        map::{MapParseTo, MapToString, StripExt},
        resolutions::sum::SumLowerResolution,
    },
    data_source::kinds::local_db::parameters::update::batching::parameters::{
        Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
    },
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
};
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::Statement;

pub struct NewTransfersInterchainStatement;

impl StatementFromRange for NewTransfersInterchainStatement {
    fn get_statement_with_context(
        _cx: &UpdateContext<'_>,
        range: Option<Range<DateTime<Utc>>>,
    ) -> Statement {
        sql_with_range_filter_opt!(
            sea_orm::DbBackend::Postgres,
            r#"
                SELECT
                    m.init_timestamp::date AS date,
                    COUNT(*)::TEXT AS value
                FROM crosschain_transfers t
                INNER JOIN crosschain_messages m ON t.message_id = m.id
                WHERE true
                    {filter}
                GROUP BY m.init_timestamp::date
            "#,
            [],
            "m.init_timestamp::timestamp",
            range,
        )
    }
}

impl_db_choice!(NewTransfersInterchainStatement, UsePrimaryDB);

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
pub type NewTransfersInterchainMonthlyInt = MapParseTo<StripExt<NewTransfersInterchainMonthly>, i64>;
pub type NewTransfersInterchainYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTransfersInterchainMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart_interchain;

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
            ],
            None,
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
            ],
            None,
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
                ("2023-02-01", "8"),
            ],
            None,
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_interchain_yearly() {
        simple_test_chart_interchain::<NewTransfersInterchainYearly>(
            "update_new_transfers_interchain_yearly",
            vec![("2022-01-01", "16"), ("2023-01-01", "25")],
            None,
        )
        .await;
    }
}
