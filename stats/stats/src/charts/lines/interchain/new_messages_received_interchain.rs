//! New interchain messages received line chart (messages with dst_tx_hash set, per day).
//! When interchain_primary_id is set, filters by dst_chain_id; otherwise counts all received.

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
};
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::{DbBackend, Statement};

pub struct NewMessagesReceivedInterchainStatement;

impl StatementFromRange for NewMessagesReceivedInterchainStatement {
    fn get_statement_with_context(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTime<Utc>>>,
    ) -> Statement {
        let (chain_condition, values) = match cx.interchain_primary_id {
            Some(primary_id) => (
                format!(" AND dst_chain_id = $1"),
                vec![sea_orm::Value::BigInt(Some(primary_id as i64))],
            ),
            None => (String::new(), vec![]),
        };
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    init_timestamp::date AS date,
                    COUNT(*)::TEXT AS value
                FROM crosschain_messages
                WHERE dst_tx_hash IS NOT NULL {chain_condition} {filter}
                GROUP BY init_timestamp::date
            "#,
            values,
            "init_timestamp::timestamp",
            range,
            chain_condition = chain_condition,
        )
    }
}

impl_db_choice!(NewMessagesReceivedInterchainStatement, UsePrimaryDB);

pub type NewMessagesReceivedInterchainRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        NewMessagesReceivedInterchainStatement,
        NaiveDate,
        String,
        QueryFullIndexerTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newMessagesReceivedInterchain".into()
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

pub type NewMessagesReceivedInterchain =
    DirectVecLocalDbChartSource<NewMessagesReceivedInterchainRemote, Batch30Days, Properties>;
pub type NewMessagesReceivedInterchainInt =
    MapParseTo<StripExt<NewMessagesReceivedInterchain>, i64>;
pub type NewMessagesReceivedInterchainWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewMessagesReceivedInterchainInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewMessagesReceivedInterchainMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewMessagesReceivedInterchainInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewMessagesReceivedInterchainMonthlyInt =
    MapParseTo<StripExt<NewMessagesReceivedInterchainMonthly>, i64>;
pub type NewMessagesReceivedInterchainYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewMessagesReceivedInterchainMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart_interchain;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_received_interchain() {
        simple_test_chart_interchain::<NewMessagesReceivedInterchain>(
            "update_new_messages_received_interchain",
            vec![
                ("2022-12-20", "1"),
                ("2022-12-21", "1"),
                ("2022-12-23", "1"),
                ("2022-12-27", "2"),
                ("2023-01-01", "1"),
                ("2023-01-02", "1"),
                ("2023-01-04", "1"),
                ("2023-01-11", "1"),
                ("2023-01-21", "2"),
                ("2023-02-01", "1"),
                ("2023-02-05", "1"),
            ],
            None,
        )
        .await;

        simple_test_chart_interchain::<NewMessagesReceivedInterchain>(
            "update_new_messages_received_interchain_primary_1",
            vec![
                ("2022-12-23", "1"),
                ("2022-12-27", "1"),
                ("2023-01-02", "1"),
                ("2023-01-21", "2"),
                ("2023-02-05", "1"),
            ],
            Some(1),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_received_interchain_weekly() {
        simple_test_chart_interchain::<NewMessagesReceivedInterchainWeekly>(
            "update_new_messages_received_interchain_weekly",
            vec![
                ("2022-12-19", "3"),
                ("2022-12-26", "3"),
                ("2023-01-02", "2"),
                ("2023-01-09", "1"),
                ("2023-01-16", "2"),
                ("2023-01-30", "2"),
            ],
            None,
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_received_interchain_monthly() {
        simple_test_chart_interchain::<NewMessagesReceivedInterchainMonthly>(
            "update_new_messages_received_interchain_monthly",
            vec![
                ("2022-12-01", "5"),
                ("2023-01-01", "6"),
                ("2023-02-01", "2"),
            ],
            None,
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_received_interchain_yearly() {
        simple_test_chart_interchain::<NewMessagesReceivedInterchainYearly>(
            "update_new_messages_received_interchain_yearly",
            vec![("2022-01-01", "5"), ("2023-01-01", "8")],
            None,
        )
        .await;
    }
}
