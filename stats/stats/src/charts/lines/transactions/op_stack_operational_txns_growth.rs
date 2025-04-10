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

use super::new_txns::op_stack_operational::OpStackNewOperationalTxnsInt;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "opStackOperationalTxnsGrowth".into()
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

pub type OpStackOperationalTxnsGrowth =
    DailyCumulativeLocalDbChartSource<OpStackNewOperationalTxnsInt, Properties>;
pub type OpStackOperationalTxnsGrowthInt = MapParseTo<StripExt<OpStackOperationalTxnsGrowth>, i64>;
pub type OpStackOperationalTxnsGrowthWeekly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<OpStackOperationalTxnsGrowthInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type OpStackOperationalTxnsGrowthMonthly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<OpStackOperationalTxnsGrowthInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type OpStackOperationalTxnsGrowthMonthlyInt =
    MapParseTo<StripExt<OpStackOperationalTxnsGrowthMonthly>, i64>;
pub type OpStackOperationalTxnsGrowthYearly = DirectVecLocalDbChartSource<
    MapToString<LastValueLowerResolution<OpStackOperationalTxnsGrowthMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_op_stack_operational_txns_growth() {
        simple_test_chart::<OpStackOperationalTxnsGrowth>(
            "update_op_stack_operational_txns_growth",
            vec![
                ("2022-11-09", "6"),
                ("2022-11-10", "20"),
                ("2022-11-11", "36"),
                ("2022-11-12", "42"),
                ("2022-12-01", "48"),
                ("2023-01-01", "49"),
                ("2023-02-01", "54"),
                ("2023-03-01", "55"),
            ],
        )
        .await;
    }

    // the implementation is generic over resolutions,
    // therefore other res should also work fine
    // (tests are becoming excruciatingly slow)
}
