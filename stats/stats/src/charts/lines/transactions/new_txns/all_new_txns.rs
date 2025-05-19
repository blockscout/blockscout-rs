use crate::{
    data_source::kinds::{
        data_manipulation::{
            map::{Map, MapParseTo, MapToString, StripExt},
            resolutions::sum::SumLowerResolution,
        },
        local_db::{
            parameters::update::batching::parameters::{
                Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
            },
            DirectVecLocalDbChartSource,
        },
    },
    define_and_impl_resolution_properties,
    types::{
        new_txns::ExtractAllTxns,
        timespans::{Month, Week, Year},
    },
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::NewTxnsCombinedRemote;

pub type NewTxnsRemote = Map<NewTxnsCombinedRemote, ExtractAllTxns>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newTxns".into()
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

pub type NewTxns = DirectVecLocalDbChartSource<NewTxnsRemote, Batch30Days, Properties>;
pub type NewTxnsInt = MapParseTo<StripExt<NewTxns>, i64>;
pub type NewTxnsWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTxnsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewTxnsMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTxnsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewTxnsMonthlyInt = MapParseTo<StripExt<NewTxnsMonthly>, i64>;
pub type NewTxnsYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTxnsMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::{
        ranged_test_chart_with_migration_variants, simple_test_chart_with_migration_variants,
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns() {
        simple_test_chart_with_migration_variants::<NewTxns>(
            "update_new_txns",
            vec![
                ("2022-11-09", "6"),
                ("2022-11-10", "14"),
                ("2022-11-11", "16"),
                ("2022-11-12", "6"),
                ("2022-12-01", "6"),
                ("2023-01-01", "1"),
                ("2023-02-01", "5"),
                ("2023-03-01", "2"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns_weekly() {
        simple_test_chart_with_migration_variants::<NewTxnsWeekly>(
            "update_new_txns_weekly",
            vec![
                ("2022-11-07", "42"),
                ("2022-11-28", "6"),
                ("2022-12-26", "1"),
                ("2023-01-30", "5"),
                ("2023-02-27", "2"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns_monthly() {
        simple_test_chart_with_migration_variants::<NewTxnsMonthly>(
            "update_new_txns_monthly",
            vec![
                ("2022-11-01", "42"),
                ("2022-12-01", "6"),
                ("2023-01-01", "1"),
                ("2023-02-01", "5"),
                ("2023-03-01", "2"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns_yearly() {
        simple_test_chart_with_migration_variants::<NewTxnsYearly>(
            "update_new_txns_yearly",
            vec![("2022-01-01", "48"), ("2023-01-01", "8")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn ranged_update_new_txns() {
        ranged_test_chart_with_migration_variants::<NewTxns>(
            "ranged_update_new_txns",
            vec![
                ("2022-11-09", "6"),
                ("2022-11-10", "14"),
                ("2022-11-11", "16"),
                ("2022-11-12", "6"),
                ("2022-12-01", "6"),
            ],
            "2022-11-08".parse().unwrap(),
            "2022-12-01".parse().unwrap(),
            None,
        )
        .await;
    }
}
