use std::ops::Range;

use crate::{
    charts::db_interaction::read::QueryAllBlockTimestampRange,
    data_source::{
        kinds::{
            data_manipulation::{
                map::{Map, MapFunction, StripExt},
                resolutions::last_value::LastValueLowerResolution,
            },
            local_db::{
                parameters::update::batching::parameters::{
                    Batch30Weeks, Batch30Years, Batch36Months,
                },
                DailyCumulativeLocalDbChartSource, DirectVecLocalDbChartSource,
            },
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        },
        types::BlockscoutMigrations,
    },
    define_and_impl_resolution_properties,
    types::timespans::{DateValue, Month, Week, Year},
    utils::sql_with_range_filter_opt,
    ChartProperties, MissingDatePolicy, Named, UpdateError,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use rust_decimal::Decimal;
use sea_orm::{DbBackend, Statement};

pub struct GasUsedPartialStatement;

impl StatementFromRange for GasUsedPartialStatement {
    fn get_statement(range: Option<Range<DateTime<Utc>>>, _: &BlockscoutMigrations) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT 
                    DATE(blocks.timestamp) as date, 
                    (sum(sum(blocks.gas_used)) OVER (ORDER BY date(blocks.timestamp))) AS value
                FROM blocks
                WHERE 
                    blocks.timestamp != to_timestamp(0) AND 
                    blocks.consensus = true {filter}
                GROUP BY date(blocks.timestamp)
                ORDER BY date;
            "#,
            [],
            "blocks.timestamp",
            range
        )
    }
}

pub type GasUsedPartialRemote = RemoteDatabaseSource<
    PullAllWithAndSort<GasUsedPartialStatement, NaiveDate, Decimal, QueryAllBlockTimestampRange>,
>;

pub struct IncrementsFromPartialSum;

impl MapFunction<Vec<DateValue<Decimal>>> for IncrementsFromPartialSum {
    type Output = Vec<DateValue<Decimal>>;
    fn function(inner_data: Vec<DateValue<Decimal>>) -> Result<Self::Output, UpdateError> {
        Ok(inner_data
            .into_iter()
            .scan(Decimal::ZERO, |state, mut next| {
                let next_diff = next.value.saturating_sub(*state);
                *state = next.value;
                next.value = next_diff;
                Some(next)
            })
            .collect())
    }
}

pub type NewGasUsedRemote = Map<GasUsedPartialRemote, IncrementsFromPartialSum>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "gasUsedGrowth".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
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

pub type GasUsedGrowth = DailyCumulativeLocalDbChartSource<NewGasUsedRemote, Properties>;
type GasUsedGrowthS = StripExt<GasUsedGrowth>;
pub type GasUsedGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<GasUsedGrowthS, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type GasUsedGrowthMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<GasUsedGrowthS, Month>,
    Batch36Months,
    MonthlyProperties,
>;
type GasUsedGrowthMonthlyS = StripExt<GasUsedGrowthMonthly>;
pub type GasUsedGrowthYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<GasUsedGrowthMonthlyS, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::{ranged_test_chart, simple_test_chart};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_gas_used_growth() {
        simple_test_chart::<GasUsedGrowth>(
            "update_gas_used_growth",
            vec![
                ("2022-11-09", "10000"),
                ("2022-11-10", "91780"),
                ("2022-11-11", "221640"),
                ("2022-11-12", "250680"),
                ("2022-12-01", "288350"),
                ("2023-01-01", "334650"),
                ("2023-02-01", "389580"),
                ("2023-03-01", "403140"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_gas_used_growth_weekly() {
        simple_test_chart::<GasUsedGrowthWeekly>(
            "update_gas_used_growth_weekly",
            vec![
                ("2022-11-07", "250680"),
                ("2022-11-28", "288350"),
                ("2022-12-26", "334650"),
                ("2023-01-30", "389580"),
                ("2023-02-27", "403140"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_gas_used_growth_monthly() {
        simple_test_chart::<GasUsedGrowthMonthly>(
            "update_gas_used_growth_monthly",
            vec![
                ("2022-11-01", "250680"),
                ("2022-12-01", "288350"),
                ("2023-01-01", "334650"),
                ("2023-02-01", "389580"),
                ("2023-03-01", "403140"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_gas_used_growth_yearly() {
        simple_test_chart::<GasUsedGrowthYearly>(
            "update_gas_used_growth_yearly",
            vec![("2022-01-01", "288350"), ("2023-01-01", "403140")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn ranged_update_gas_used_growth() {
        let value_2022_11_12 = "250680";
        ranged_test_chart::<GasUsedGrowth>(
            "ranged_update_gas_used_growth",
            vec![("2022-11-20", value_2022_11_12), ("2022-12-01", "288350")],
            "2022-11-20".parse().unwrap(),
            "2022-12-01".parse().unwrap(),
            None,
        )
        .await;
    }
}
