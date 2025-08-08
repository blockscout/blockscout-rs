use crate::{
    IndexingStatus, MissingDatePolicy, Named,
    charts::chart::ChartProperties,
    data_source::kinds::{
        data_manipulation::{map::StripExt, resolutions::last_value::LastValueLowerResolution},
        local_db::{
            DailyCumulativeLocalDbChartSource, DirectVecLocalDbChartSource,
            parameters::update::batching::parameters::{Batch30Weeks, Batch30Years, Batch36Months},
        },
    },
    define_and_impl_resolution_properties,
    indexing_status::{IndexingStatusTrait, ZetachainCctxIndexingStatus},
    types::timespans::{Month, Week, Year},
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

use super::new_cc_txns::NewZetachainCrossChainTxnsInt;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "zetachainCrossChainTxnsGrowth".into()
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
        IndexingStatus::LEAST_RESTRICTIVE
            .with_zetachain_cctx(ZetachainCctxIndexingStatus::IndexedHistoricalData)
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

pub type ZetachainCrossChainTxnsGrowth =
    DailyCumulativeLocalDbChartSource<NewZetachainCrossChainTxnsInt, Properties>;
type ZetachainCrossChainTxnsGrowthS = StripExt<ZetachainCrossChainTxnsGrowth>;
pub type ZetachainCrossChainTxnsGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<ZetachainCrossChainTxnsGrowthS, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type ZetachainCrossChainTxnsGrowthMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<ZetachainCrossChainTxnsGrowthS, Month>,
    Batch36Months,
    MonthlyProperties,
>;
type ZetachainCrossChainTxnsGrowthMonthlyS = StripExt<ZetachainCrossChainTxnsGrowthMonthly>;
pub type ZetachainCrossChainTxnsGrowthYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<ZetachainCrossChainTxnsGrowthMonthlyS, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_zetachain_cross_chain_txns_growth() {
        simple_test_chart::<ZetachainCrossChainTxnsGrowth>(
            "update_zetachain_cross_chain_txns_growth",
            vec![("2022-11-09", "1"), ("2022-11-10", "3")],
        )
        .await;
    }
}
