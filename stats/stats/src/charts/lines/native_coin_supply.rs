use std::ops::Range;

use crate::{
    charts::db_interaction::read::QueryAllBlockTimestampRange,
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapToString, StripExt},
                resolutions::last_value::LastValueLowerResolution,
            },
            local_db::{
                parameters::update::batching::parameters::{
                    Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
                },
                DirectVecLocalDbChartSource,
            },
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        },
        types::BlockscoutMigrations,
    },
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    ChartProperties, Named,
};

use chrono::NaiveDate;
use chrono::{DateTime, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

const ETH: i64 = 1_000_000_000_000_000_000;

pub struct NativeCoinSupplyStatement;

impl StatementFromRange for NativeCoinSupplyStatement {
    fn get_statement(range: Option<Range<DateTime<Utc>>>, _: &BlockscoutMigrations) -> Statement {
        let day_range: Option<Range<NaiveDate>> = range.map(|r| {
            let Range { start, end } = r;
            // chart is off anyway, so shouldn't be a big deal
            start.date_naive()..end.date_naive()
        });
        // query uses date, therefore `sql_with_range_filter_opt` does not quite fit
        // (making it parameter-agnostic seems not straightforward, let's keep it as-is)
        match day_range {
            Some(range) => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r"
                    SELECT date, value FROM
                    (
                        SELECT
                            day as date,
                            (sum(
                                CASE
                                    WHEN address_hash = '\x0000000000000000000000000000000000000000' THEN -value
                                    ELSE value
                                END
                            ) / $1)::float AS value
                        FROM address_coin_balances_daily
                        WHERE  day != to_timestamp(0) AND
                                            day <= $3 AND
                                            day >= $2
                        GROUP BY day
                    ) as intermediate
                    WHERE value is not NULL;
                ",
                vec![ETH.into(), range.start.into(), range.end.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r"
                    SELECT date, value FROM
                    (
                        SELECT
                            day as date,
                            (sum(
                                CASE
                                    WHEN address_hash = '\x0000000000000000000000000000000000000000' THEN -value
                                    ELSE value
                                END
                            ) / $1)::float AS value
                        FROM address_coin_balances_daily
                        WHERE  day != to_timestamp(0)
                        GROUP BY day
                    ) as intermediate
                    WHERE value is not NULL;
                ",
                vec![ETH.into()],
            ),
        }
    }
}

// query returns float value
pub type NativeCoinSupplyRemote = RemoteDatabaseSource<
    PullAllWithAndSort<NativeCoinSupplyStatement, NaiveDate, f64, QueryAllBlockTimestampRange>,
>;

pub type NativeCoinSupplyRemoteString = MapToString<NativeCoinSupplyRemote>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "nativeCoinSupply".into()
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

pub type NativeCoinSupply =
    DirectVecLocalDbChartSource<NativeCoinSupplyRemoteString, Batch30Days, Properties>;
type NativeCoinSupplyS = StripExt<NativeCoinSupply>;
pub type NativeCoinSupplyWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<NativeCoinSupplyS, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NativeCoinSupplyMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<NativeCoinSupplyS, Month>,
    Batch36Months,
    MonthlyProperties,
>;
type NativeCoinSupplyMonthlyS = StripExt<NativeCoinSupplyMonthly>;
pub type NativeCoinSupplyYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<NativeCoinSupplyMonthlyS, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coin_supply() {
        simple_test_chart::<NativeCoinSupply>(
            "update_native_coin_supply",
            vec![
                ("2022-11-09", "6666.666666666667"),
                ("2022-11-10", "6000"),
                ("2022-11-11", "5000"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coin_supply_weekly() {
        simple_test_chart::<NativeCoinSupplyWeekly>(
            "update_native_coin_supply_weekly",
            vec![("2022-11-07", "5000")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coin_supply_monthly() {
        simple_test_chart::<NativeCoinSupplyMonthly>(
            "update_native_coin_supply_monthly",
            vec![("2022-11-01", "5000")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coin_supply_yearly() {
        simple_test_chart::<NativeCoinSupplyYearly>(
            "update_native_coin_supply_yearly",
            vec![("2022-01-01", "5000")],
        )
        .await;
    }
}
