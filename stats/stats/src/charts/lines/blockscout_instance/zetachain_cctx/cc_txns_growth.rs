use crate::chart_prelude::*;

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
    use crate::tests::simple_test::simple_test_chart_with_zetachain_cctx;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_zetachain_cross_chain_txns_growth() {
        simple_test_chart_with_zetachain_cctx::<ZetachainCrossChainTxnsGrowth>(
            "update_zetachain_cross_chain_txns_growth",
            vec![("2022-11-09", "1"), ("2022-11-10", "3")],
        )
        .await;
    }
}
