// SPDX-License-Identifier: LicenseRef-Blockscout

//! Cumulative total of chain-wide fees on Filecoin, in FIL (REV-style:
//! `burn + miner tips`). Running total of `filecoin_new_chain_fees`.

use crate::chart_prelude::*;

use super::filecoin_new_chain_fees::FilecoinNewChainFeesFloat;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "filecoinChainFeesGrowth".into()
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

pub type FilecoinChainFeesGrowth =
    DailyCumulativeLocalDbChartSource<FilecoinNewChainFeesFloat, Properties>;
type FilecoinChainFeesGrowthS = StripExt<FilecoinChainFeesGrowth>;

pub type FilecoinChainFeesGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<FilecoinChainFeesGrowthS, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type FilecoinChainFeesGrowthMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<FilecoinChainFeesGrowthS, Month>,
    Batch36Months,
    MonthlyProperties,
>;
type FilecoinChainFeesGrowthMonthlyS = StripExt<FilecoinChainFeesGrowthMonthly>;
pub type FilecoinChainFeesGrowthYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<FilecoinChainFeesGrowthMonthlyS, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart_filecoin;

    // Running totals = cumsum of the `filecoinNewChainFees` values over the
    // Filecoin fixture layer. The genuine no-data day (`2022-12-15`) records
    // no chart_data row, so it is absent from the unfilled output — the
    // surrounding days (`2022-12-01`, `2023-01-01`) pin that the running
    // total carried across the gap correctly; the filled "previous day's
    // cumulative" behavior is asserted at the API level. `2022-11-11`
    // (tips-only) and `2023-03-01` (burn-only) advance the total by exactly
    // that single contribution.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_filecoin_chain_fees_growth() {
        simple_test_chart_filecoin::<FilecoinChainFeesGrowth>(
            "update_filecoin_chain_fees_growth",
            vec![
                ("2022-11-09", "30000000"),
                ("2022-11-10", "30001000.000584006"),
                ("2022-11-11", "30001000.00177722"),
                ("2022-11-12", "30003500.00256677"),
                ("2022-12-01", "30010000.003450688"),
                ("2023-01-01", "30020000.00347218"),
                ("2023-02-01", "30035000.004523344"),
                ("2023-03-01", "30050000.004523344"),
            ],
        )
        .await;
    }

    // the implementation is generic over resolutions,
    // therefore other res should also work fine
    // (tests are becoming excruciatingly slow)
}
