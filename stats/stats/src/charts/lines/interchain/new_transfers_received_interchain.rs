//! New interchain transfers received line chart (transfers whose message has dst_tx_hash set, per day).
//! When interchain_primary_id is set, filters by message's dst_chain_id; otherwise counts all received.

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

pub struct NewTransfersReceivedInterchainStatement;

impl StatementFromRange for NewTransfersReceivedInterchainStatement {
    fn get_statement_with_context(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTime<Utc>>>,
    ) -> Statement {
        let (chain_condition, mut values): (String, Vec<sea_orm::Value>) =
            match cx.interchain_primary_id {
                Some(primary_id) => (
                    " AND m.dst_chain_id = $1".into(),
                    vec![sea_orm::Value::BigInt(Some(primary_id as i64))],
                ),
                None => (String::new(), vec![]),
            };
        let filter_arg_start = values.len() + 1;
        let (range_filter, range_values) =
            produce_filter_and_values(range, "m.init_timestamp::timestamp", filter_arg_start);
        values.extend(range_values);
        let sql = format!(
            r#"
                SELECT
                    m.init_timestamp::date AS date,
                    COUNT(*)::TEXT AS value
                FROM crosschain_transfers t
                INNER JOIN crosschain_messages m ON t.message_id = m.id
                WHERE m.dst_tx_hash IS NOT NULL{chain_condition}{range_filter}
                GROUP BY m.init_timestamp::date
            "#
        );
        Statement::from_sql_and_values(DbBackend::Postgres, &sql, values)
    }
}

impl_db_choice!(NewTransfersReceivedInterchainStatement, UsePrimaryDB);

pub type NewTransfersReceivedInterchainRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        NewTransfersReceivedInterchainStatement,
        NaiveDate,
        String,
        QueryFullIndexerTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newTransfersReceivedInterchain".into()
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

pub type NewTransfersReceivedInterchain =
    DirectVecLocalDbChartSource<NewTransfersReceivedInterchainRemote, Batch30Days, Properties>;
pub type NewTransfersReceivedInterchainInt =
    MapParseTo<StripExt<NewTransfersReceivedInterchain>, i64>;
pub type NewTransfersReceivedInterchainWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTransfersReceivedInterchainInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewTransfersReceivedInterchainMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTransfersReceivedInterchainInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewTransfersReceivedInterchainMonthlyInt =
    MapParseTo<StripExt<NewTransfersReceivedInterchainMonthly>, i64>;
pub type NewTransfersReceivedInterchainYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTransfersReceivedInterchainMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart_interchain;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_received_interchain() {
        simple_test_chart_interchain::<NewTransfersReceivedInterchain>(
            "update_new_transfers_received_interchain",
            vec![
                ("2022-12-20", "2"),
                ("2022-12-23", "1"),
                ("2022-12-27", "5"),
                ("2023-01-01", "2"),
                ("2023-01-04", "3"),
                ("2023-01-11", "1"),
                ("2023-01-21", "3"),
                ("2023-02-01", "2"),
                ("2023-02-05", "1"),
            ],
            None,
        )
        .await;

        simple_test_chart_interchain::<NewTransfersReceivedInterchain>(
            "update_new_transfers_received_interchain_primary_1",
            vec![
                ("2022-12-23", "1"),
                ("2022-12-27", "1"),
                ("2023-01-21", "3"),
                ("2023-02-05", "1"),
            ],
            Some(1),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_received_interchain_weekly() {
        simple_test_chart_interchain::<NewTransfersReceivedInterchainWeekly>(
            "update_new_transfers_received_interchain_weekly",
            vec![
                ("2022-12-19", "3"),
                ("2022-12-26", "7"),
                ("2023-01-02", "3"),
                ("2023-01-09", "1"),
                ("2023-01-16", "3"),
                ("2023-01-30", "3"),
            ],
            None,
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_received_interchain_monthly() {
        simple_test_chart_interchain::<NewTransfersReceivedInterchainMonthly>(
            "update_new_transfers_received_interchain_monthly",
            vec![
                ("2022-12-01", "8"),
                ("2023-01-01", "9"),
                ("2023-02-01", "3"),
            ],
            None,
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_received_interchain_yearly() {
        simple_test_chart_interchain::<NewTransfersReceivedInterchainYearly>(
            "update_new_transfers_received_interchain_yearly",
            vec![("2022-01-01", "8"), ("2023-01-01", "12")],
            None,
        )
        .await;
    }
}
