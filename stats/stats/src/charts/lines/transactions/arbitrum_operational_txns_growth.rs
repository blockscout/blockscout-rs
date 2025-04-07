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

use super::arbitrum_new_operational_txns::ArbitrumNewOperationalTxns;

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

pub type ArbitrumOperationalTxnsGrowth = DailyCumulativeLocalDbChartSource<
    MapParseTo<StripExt<ArbitrumNewOperationalTxns>, i64>,
    Properties,
>;
pub type ArbitrumOperationalTxnsGrowthInt =
    MapParseTo<StripExt<ArbitrumOperationalTxnsGrowth>, i64>;
pub type ArbitrumOperationalTxnsGrowthWeekly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<ArbitrumOperationalTxnsGrowthInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type ArbitrumOperationalTxnsGrowthMonthly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<ArbitrumOperationalTxnsGrowthInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type ArbitrumOperationalTxnsGrowthMonthlyInt =
    MapParseTo<StripExt<ArbitrumOperationalTxnsGrowthMonthly>, i64>;
pub type ArbitrumOperationalTxnsGrowthYearly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<ArbitrumOperationalTxnsGrowthMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_arbitrum_operational_txns_growth() {
        simple_test_chart::<ArbitrumOperationalTxnsGrowth>(
            "update_arbitrum_operational_txns_growth",
            vec![
                ("2022-11-09", "5"),
                ("2022-11-10", "16"),
                ("2022-11-11", "28"),
                ("2022-11-12", "33"),
                ("2022-12-01", "38"),
                ("2023-01-01", "38"),
                ("2023-02-01", "42"),
                ("2023-03-01", "43"),
            ],
        )
        .await;
    }

    // the implementation is generic over resolutions,
    // therefore other res should also work fine
    // (tests are becoming excruciatingly slow)
}
