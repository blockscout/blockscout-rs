//! New interchain messages sent line chart (messages with src_tx_hash set, per day).
//! When interchain_primary_id is set, filters by src_chain_id; otherwise counts all sent.

use std::ops::Range;

use crate::{
    chart_prelude::*,
    charts::db_interaction::read::QueryFullIndexerTimestampRange,
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString, StripExt},
                resolutions::sum::SumLowerResolution,
            },
            local_db::parameters::update::batching::parameters::{
                Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
            },
        },
        types::UpdateContext,
    },
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    utils::produce_filter_and_values,
};
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::{DbBackend, Statement};

pub struct NewMessagesSentInterchainStatement;

impl StatementFromRange for NewMessagesSentInterchainStatement {
    fn get_statement_with_context(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTime<Utc>>>,
    ) -> Statement {
        let (chain_condition, mut values): (String, Vec<sea_orm::Value>) =
            match cx.interchain_primary_id {
                Some(primary_id) => (
                    " AND src_chain_id = $1".into(),
                    vec![sea_orm::Value::BigInt(Some(primary_id as i64))],
                ),
                None => (String::new(), vec![]),
            };
        let filter_arg_start = values.len() + 1;
        let (range_filter, range_values) =
            produce_filter_and_values(range, "init_timestamp::timestamp", filter_arg_start);
        values.extend(range_values);
        let sql = format!(
            r#"
                SELECT
                    init_timestamp::date AS date,
                    COUNT(*)::TEXT AS value
                FROM crosschain_messages
                WHERE src_tx_hash IS NOT NULL{chain_condition}{range_filter}
                GROUP BY init_timestamp::date
            "#
        );
        Statement::from_sql_and_values(DbBackend::Postgres, &sql, values)
    }
}

impl_db_choice!(NewMessagesSentInterchainStatement, UsePrimaryDB);

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
    use crate::tests::simple_test::simple_test_chart_interchain;

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
                ("2023-02-10", "1"),
            ],
            None,
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
                ("2023-02-10", "1"),
            ],
            Some(1),
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
                ("2023-02-06", "1"),
            ],
            None,
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
                ("2023-02-06", "1"),
            ],
            Some(1),
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
                ("2023-02-01", "3"),
            ],
            None,
        )
        .await;

        simple_test_chart_interchain::<NewMessagesSentInterchainMonthly>(
            "update_new_messages_sent_interchain_monthly_primary_1",
            vec![
                ("2022-12-01", "3"),
                ("2023-01-01", "5"),
                ("2023-02-01", "3"),
            ],
            Some(1),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_sent_interchain_yearly() {
        simple_test_chart_interchain::<NewMessagesSentInterchainYearly>(
            "update_new_messages_sent_interchain_yearly",
            vec![("2022-01-01", "5"), ("2023-01-01", "10")],
            None,
        )
        .await;

        simple_test_chart_interchain::<NewMessagesSentInterchainYearly>(
            "update_new_messages_sent_interchain_yearly_primary_1",
            vec![("2022-01-01", "3"), ("2023-01-01", "8")],
            Some(1),
        )
        .await;
    }
}
