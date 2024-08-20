use std::ops::Range;

use crate::{
    charts::types::timespans::DateValue,
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString},
                resolutions::sum::SumLowerResolution,
            },
            local_db::{
                parameters::update::batching::parameters::{
                    Batch30Weeks, Batch30Years, Batch36Months, BatchMaxDays,
                },
                DirectVecLocalDbChartSource,
            },
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour, StatementFromRange},
        },
        UpdateContext,
    },
    define_and_impl_resolution_properties,
    missing_date::trim_out_of_range_sorted,
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
    ChartProperties, Named, UpdateError,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

pub struct NewAccountsStatement;

impl StatementFromRange for NewAccountsStatement {
    fn get_statement(range: Option<Range<DateTimeUtc>>) -> Statement {
        // `MIN_UTC` does not fit into postgres' timestamp. Unix epoch start should be enough
        let min_timestamp = DateTimeUtc::UNIX_EPOCH;
        // All transactions from the beginning must be considered to calculate new accounts correctly.
        // E.g. if account was first active both before `range.start()` and within the range,
        // we don't want to count it within the range (as it's not a *new* account).
        let range = range.map(|r| (min_timestamp..r.end));
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    first_tx.date as date,
                    count(*)::TEXT as value
                FROM (
                    SELECT DISTINCT ON (t.from_address_hash)
                        b.timestamp::date as date
                    FROM transactions  t
                    JOIN blocks        b ON t.block_hash = b.hash
                    WHERE
                        b.timestamp != to_timestamp(0) AND
                        b.consensus = true {filter}
                    ORDER BY t.from_address_hash, b.timestamp
                ) first_tx
                GROUP BY first_tx.date;
            "#,
            [],
            "b.timestamp",
            range
        )
    }
}

pub struct NewAccountsQueryBehaviour;

impl RemoteQueryBehaviour for NewAccountsQueryBehaviour {
    type Output = Vec<DateValue<String>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTimeUtc>>,
    ) -> Result<Vec<DateValue<String>>, UpdateError> {
        let query = NewAccountsStatement::get_statement(range.clone());
        let mut data = DateValue::<String>::find_by_statement(query)
            .all(cx.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        // make sure that it's sorted
        data.sort_by_key(|d| d.timespan);
        if let Some(range) = range {
            let range = range.start.date_naive()..=range.end.date_naive();
            trim_out_of_range_sorted(&mut data, range);
        }
        Ok(data)
    }
}

/// Note:  The intended strategy is to update whole range at once, even
/// though the implementation allows batching. The batching was done
/// to simplify interface of the data source.
///
/// Thus, use max batch size in the dependant data sources.
pub type NewAccountsRemote = RemoteDatabaseSource<NewAccountsQueryBehaviour>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newAccounts".into()
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

pub type NewAccounts = DirectVecLocalDbChartSource<NewAccountsRemote, BatchMaxDays, Properties>;
pub type NewAccountsInt = MapParseTo<NewAccounts, i64>;
pub type NewAccountsWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewAccountsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewAccountsMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewAccountsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewAccountsMonthlyInt = MapParseTo<NewAccountsMonthly, i64>;
pub type NewAccountsYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewAccountsMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::{ranged_test_chart, simple_test_chart};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_accounts() {
        simple_test_chart::<NewAccounts>(
            "update_new_accounts",
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "3"),
                ("2022-11-11", "4"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_accounts_weekly() {
        simple_test_chart::<NewAccountsWeekly>(
            "update_new_accounts_weekly",
            vec![("2022-11-07", "8"), ("2023-02-27", "1")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_accounts_monthly() {
        simple_test_chart::<NewAccountsMonthly>(
            "update_new_accounts_monthly",
            vec![("2022-11-01", "8"), ("2023-03-01", "1")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_accounts_yearly() {
        simple_test_chart::<NewAccountsYearly>(
            "update_new_accounts_yearly",
            vec![("2022-01-01", "8"), ("2023-01-01", "1")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn ranged_update_new_accounts() {
        ranged_test_chart::<NewAccounts>(
            "ranged_update_new_accounts",
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "3"),
                // only half of the tx's should be detected; others were created
                // later than update time
                ("2022-11-11", "2"),
            ],
            "2022-11-09".parse().unwrap(),
            "2022-11-11".parse().unwrap(),
            Some("2022-11-11T14:00:00".parse().unwrap()),
        )
        .await;
    }
}
