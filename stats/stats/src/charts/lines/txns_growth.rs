use crate::{
    charts::chart::ChartProperties,
    data_source::kinds::{
        data_manipulation::resolutions::last_value::LastValueLowerResolution,
        local_db::{
            parameters::update::batching::parameters::{Batch30Weeks, Batch30Years, Batch36Months},
            DailyCumulativeLocalDbChartSource, DirectVecLocalDbChartSource,
        },
    },
    delegated_properties_with_resolutions,
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

delegated_properties_with_resolutions!(
    delegate: {
        WeeklyProperties: Week,
        MonthlyProperties: Month,
        YearlyProperties: Year,
    }
    ..Properties
);

pub type TxnsGrowth = DailyCumulativeLocalDbChartSource<NewTxnsInt, Properties>;
pub type TxnsGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<TxnsGrowth, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type TxnsGrowthMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<TxnsGrowth, Month>,
    Batch36Months,
    MonthlyProperties,
>;
pub type TxnsGrowthYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<TxnsGrowthMonthly, Year>,
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
