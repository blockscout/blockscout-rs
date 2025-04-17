//! Cumulative total number of accounts in the network.

use super::new_builder_accounts::NewBuilderAccountsInt;
use crate::{
    data_source::kinds::{
        data_manipulation::{map::StripExt, resolutions::last_value::LastValueLowerResolution},
        local_db::{
            parameters::update::batching::parameters::{Batch30Weeks, Batch30Years, Batch36Months},
            DailyCumulativeLocalDbChartSource, DirectVecLocalDbChartSource,
        },
    },
    define_and_impl_resolution_properties,
    indexing_status::{BlockscoutIndexingStatus, UserOpsIndexingStatus},
    types::timespans::{Month, Week, Year},
    ChartProperties, IndexingStatus, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "builderAccountsGrowth".into()
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

    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus {
            blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
            // don't need user ops at all
            user_ops: UserOpsIndexingStatus::IndexingPastOperations,
        }
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

pub type BuilderAccountsGrowth =
    DailyCumulativeLocalDbChartSource<NewBuilderAccountsInt, Properties>;
type BuilderAccountsGrowthS = StripExt<BuilderAccountsGrowth>;

pub type BuilderAccountsGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<BuilderAccountsGrowthS, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type BuilderAccountsGrowthMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<BuilderAccountsGrowthS, Month>,
    Batch36Months,
    MonthlyProperties,
>;
type BuilderAccountsGrowthMonthlyS = StripExt<BuilderAccountsGrowthMonthly>;
pub type BuilderAccountsGrowthYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<BuilderAccountsGrowthMonthlyS, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_builder_accounts_growth() {
        simple_test_chart::<BuilderAccountsGrowth>(
            "update_builder_accounts_growth",
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "4"),
                ("2022-11-11", "8"),
            ],
        )
        .await;
    }
}
