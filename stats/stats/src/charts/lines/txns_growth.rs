use crate::{
    charts::chart::ChartProperties,
    data_source::kinds::{
        data_manipulation::{map::StripExt, resolutions::last_value::LastValueLowerResolution},
        local_db::{
            parameters::update::batching::parameters::{Batch30Weeks, Batch30Years, Batch36Months},
            DailyCumulativeLocalDbChartSource, DirectVecLocalDbChartSource,
        },
    },
    define_and_impl_resolution_properties,
    lines::NewTxnsInt,
    types::timespans::{Month, Week, Year},
    MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "txnsGrowth".into()
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

pub type TxnsGrowth = DailyCumulativeLocalDbChartSource<NewTxnsInt, Properties>;
type TxnsGrowthS = StripExt<TxnsGrowth>;
pub type TxnsGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<TxnsGrowthS, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type TxnsGrowthMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<TxnsGrowthS, Month>,
    Batch36Months,
    MonthlyProperties,
>;
type TxnsGrowthMonthlyS = StripExt<TxnsGrowthMonthly>;
pub type TxnsGrowthYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<TxnsGrowthMonthlyS, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_growth() {
        simple_test_chart::<TxnsGrowth>(
            "update_txns_growth",
            vec![
                ("2022-11-09", "5"),
                ("2022-11-10", "17"),
                ("2022-11-11", "31"),
                ("2022-11-12", "36"),
                ("2022-12-01", "41"),
                ("2023-01-01", "42"),
                ("2023-02-01", "46"),
                ("2023-03-01", "47"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_growth_weekly() {
        simple_test_chart::<TxnsGrowthWeekly>(
            "update_txns_growth_weekly",
            vec![
                ("2022-11-07", "36"),
                ("2022-11-28", "41"),
                ("2022-12-26", "42"),
                ("2023-01-30", "46"),
                ("2023-02-27", "47"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_growth_monthly() {
        simple_test_chart::<TxnsGrowthMonthly>(
            "update_txns_growth_monthly",
            vec![
                ("2022-11-01", "36"),
                ("2022-12-01", "41"),
                ("2023-01-01", "42"),
                ("2023-02-01", "46"),
                ("2023-03-01", "47"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_growth_yearly() {
        simple_test_chart::<TxnsGrowthYearly>(
            "update_txns_growth_yearly",
            vec![("2022-01-01", "41"), ("2023-01-01", "47")],
        )
        .await;
    }
}
