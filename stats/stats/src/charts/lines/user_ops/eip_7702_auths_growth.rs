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
    indexing_status::{BlockscoutIndexingStatus, UserOpsIndexingStatus},
    types::timespans::{Month, Week, Year},
    IndexingStatus, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::new_eip_7702_auths::NewEip7702AuthsInt;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "eip7702AuthsGrowth".into()
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
            blockscout: BlockscoutIndexingStatus::BlocksIndexed,
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

pub type Eip7702AuthsGrowth = DailyCumulativeLocalDbChartSource<NewEip7702AuthsInt, Properties>;
type Eip7702AuthsGrowthS = StripExt<Eip7702AuthsGrowth>;
pub type Eip7702AuthsGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<Eip7702AuthsGrowthS, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type Eip7702AuthsGrowthMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<Eip7702AuthsGrowthS, Month>,
    Batch36Months,
    MonthlyProperties,
>;
type Eip7702AuthsGrowthMonthlyS = StripExt<Eip7702AuthsGrowthMonthly>;
pub type Eip7702AuthsGrowthYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<Eip7702AuthsGrowthMonthlyS, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_eip_7702_auths_growth() {
        simple_test_chart::<Eip7702AuthsGrowth>(
            "update_eip_7702_auths_growth",
            vec![
                ("2022-11-10", "1"),
                ("2022-11-11", "3"),
                ("2023-03-01", "6"),
            ],
        )
        .await;
    }
}
