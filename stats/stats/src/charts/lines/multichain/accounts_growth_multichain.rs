use std::{collections::HashSet, ops::Range};

use crate::{
    ChartKey, ChartProperties, Named,
    charts::db_interaction::read::QueryFullIndexerTimestampRange,
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString, StripExt},
                resolutions::last_value::LastValueLowerResolution,
            },
            local_db::{
                DirectVecLocalDbChartSource,
                parameters::update::batching::parameters::{
                    Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
                },
            },
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        },
        types::IndexerMigrations,
    },
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct AccountsGrowthMultichainStatement;

impl StatementFromRange for AccountsGrowthMultichainStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _: &IndexerMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    c.date,
                    SUM(c.total_addresses_number)::TEXT AS value
                FROM counters_global_imported as c
                WHERE
                    c.total_addresses_number IS NOT NULL
                    {filter}
                GROUP BY date
            "#,
            [],
            "c.date::timestamp",
            range
        )
    }
}

pub type AccountsGrowthMultichainRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        AccountsGrowthMultichainStatement,
        NaiveDate,
        String,
        QueryFullIndexerTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "accountsGrowthMultichain".into()
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

pub type AccountsGrowthMultichain =
    DirectVecLocalDbChartSource<AccountsGrowthMultichainRemote, Batch30Days, Properties>;
pub type AccountsGrowthMultichainInt = MapParseTo<StripExt<AccountsGrowthMultichain>, i64>;
pub type AccountsGrowthMultichainWeekly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<AccountsGrowthMultichainInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type AccountsGrowthMultichainMonthly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<AccountsGrowthMultichainInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type AccountsGrowthMultichainMonthlyInt =
    MapParseTo<StripExt<AccountsGrowthMultichainMonthly>, i64>;
pub type AccountsGrowthMultichainYearly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<AccountsGrowthMultichainMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart_multichain;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_accounts_growth_multichain() {
        simple_test_chart_multichain::<AccountsGrowthMultichain>(
            "update_accounts_growth_multichain",
            vec![
                ("2022-06-28", "666"),
                ("2022-07-01", "750"),
                ("2022-08-04", "825"),
                ("2022-08-05", "872"),
                ("2022-08-06", "920"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_accounts_growth_multichain_weekly() {
        simple_test_chart_multichain::<AccountsGrowthMultichainWeekly>(
            "update_accounts_growth_multichain_weekly",
            vec![("2022-06-27", "750"), ("2022-08-01", "920")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_accounts_growth_multichain_monthly() {
        simple_test_chart_multichain::<AccountsGrowthMultichainMonthly>(
            "update_accounts_growth_multichain_monthly",
            vec![
                ("2022-06-01", "666"),
                ("2022-07-01", "750"),
                ("2022-08-01", "920"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_accounts_growth_multichain_yearly() {
        simple_test_chart_multichain::<AccountsGrowthMultichainYearly>(
            "update_accounts_growth_multichain_yearly",
            vec![("2022-01-01", "920")],
        )
        .await;
    }
}
