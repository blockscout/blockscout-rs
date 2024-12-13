use crate::{
    data_source::kinds::{
        data_manipulation::{
            map::{MapParseTo, MapToString, StripExt},
            resolutions::last_value::LastValueLowerResolution,
        },
        local_db::{
            parameters::update::batching::parameters::{Batch30Weeks, Batch30Years, Batch36Months},
            DailyCumulativeLocalDbChartSource, DirectVecLocalDbChartSource,
        },
    },
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    ChartProperties, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::new_operational_txns::NewOperationalTxns;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "operationalTxnsGrowth".into()
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

pub type OperationalTxnsGrowth =
    DailyCumulativeLocalDbChartSource<MapParseTo<StripExt<NewOperationalTxns>, i64>, Properties>;
pub type OperationalTxnsGrowthInt = MapParseTo<StripExt<OperationalTxnsGrowth>, i64>;
pub type OperationalTxnsGrowthWeekly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<OperationalTxnsGrowthInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type OperationalTxnsGrowthMonthly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<OperationalTxnsGrowthInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type OperationalTxnsGrowthMonthlyInt = MapParseTo<StripExt<OperationalTxnsGrowthMonthly>, i64>;
pub type OperationalTxnsGrowthYearly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<OperationalTxnsGrowthMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_operational_txns_growth() {
        simple_test_chart::<OperationalTxnsGrowth>(
            "update_operational_txns_growth",
            vec![
                ("2022-11-09", "4"),
                ("2022-11-10", "13"),
                ("2022-11-11", "23"),
                ("2022-11-12", "27"),
                ("2022-12-01", "31"),
                ("2023-01-01", "31"),
                ("2023-02-01", "34"),
                ("2023-03-01", "34"),
            ],
        )
        .await;
    }

    // the implementation is generic over resolutions,
    // therefore other res should also work fine
    // (tests are becoming excruciatingly slow)
}
