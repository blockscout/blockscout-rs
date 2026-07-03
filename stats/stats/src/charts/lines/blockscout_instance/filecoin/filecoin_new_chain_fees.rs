// SPDX-License-Identifier: LicenseRef-Blockscout

//! Total chain-wide fees per day on Filecoin, in FIL (REV-style:
//! `burn + miner tips`).
//!
//! Composed from two intermediate charts:
//! - `Delta` over the f099 burn-actor balance (`burn_actor_balance`) —
//!   per-day base-fee + over-estimation burn;
//! - the per-day FEVM miner-tip sum (`fevm_fee_tips`).

use std::fmt::Debug;

use crate::chart_prelude::*;

use super::{burn_actor_balance::BurnActorBalanceFloat, fevm_fee_tips::FevmFeeTipsFloat};

use itertools::Itertools;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "filecoinNewChainFees".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }

    fn missing_date_policy() -> MissingDatePolicy {
        // a day with neither burn-actor change nor FEVM transactions
        // contributes 0 to total fees
        MissingDatePolicy::FillZero
    }
}

pub struct AddBurnAndTips;

type Input<Resolution> = (
    // burn delta
    Vec<TimespanValue<Resolution, f64>>,
    // fevm fee tips
    Vec<TimespanValue<Resolution, f64>>,
);

impl<Resolution> MapFunction<Input<Resolution>> for AddBurnAndTips
where
    Resolution: Timespan + Send + Ord + Debug,
{
    type Output = Vec<TimespanValue<Resolution, f64>>;

    fn function(inner_data: Input<Resolution>) -> Result<Self::Output, crate::ChartError> {
        let (burn_data, tips_data) = inner_data;
        let combined = zip_same_timespan(burn_data, tips_data);
        let data = combined
            .into_iter()
            .map(|(timespan, data)| {
                // a missing value means no change of the respective component
                let (burn, tips) = data.or(0.0, 0.0);
                TimespanValue {
                    timespan,
                    value: burn + tips,
                }
            })
            .collect_vec();
        Ok(data)
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

pub type FilecoinNewChainFees = DirectVecLocalDbChartSource<
    MapToString<Map<(Delta<BurnActorBalanceFloat>, FevmFeeTipsFloat), AddBurnAndTips>>,
    BatchMaxDays,
    Properties,
>;
pub type FilecoinNewChainFeesFloat = MapParseTo<StripExt<FilecoinNewChainFees>, f64>;
pub type FilecoinNewChainFeesWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<FilecoinNewChainFeesFloat, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type FilecoinNewChainFeesMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<FilecoinNewChainFeesFloat, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type FilecoinNewChainFeesMonthlyFloat = MapParseTo<StripExt<FilecoinNewChainFeesMonthly>, f64>;
pub type FilecoinNewChainFeesYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<FilecoinNewChainFeesMonthlyFloat, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::tests::{point_construction::d_v_double, simple_test::simple_test_chart_filecoin};

    #[test]
    fn add_burn_and_tips_works() {
        // both components present
        assert_eq!(
            AddBurnAndTips::function((
                vec![
                    d_v_double("2023-01-01", 100.5),
                    d_v_double("2023-01-02", 3.0)
                ],
                vec![
                    d_v_double("2023-01-01", 0.25),
                    d_v_double("2023-01-02", 1.0)
                ],
            ))
            .unwrap(),
            vec![
                d_v_double("2023-01-01", 100.75),
                d_v_double("2023-01-02", 4.0)
            ],
        );
        // a date present only in burn (tips = 0)
        assert_eq!(
            AddBurnAndTips::function((
                vec![
                    d_v_double("2023-01-01", 100.5),
                    d_v_double("2023-01-02", 3.0)
                ],
                vec![d_v_double("2023-01-02", 1.0)],
            ))
            .unwrap(),
            vec![
                d_v_double("2023-01-01", 100.5),
                d_v_double("2023-01-02", 4.0)
            ],
        );
        // a date present only in tips (burn = 0)
        assert_eq!(
            AddBurnAndTips::function((
                vec![d_v_double("2023-01-02", 3.0)],
                vec![
                    d_v_double("2023-01-01", 0.25),
                    d_v_double("2023-01-02", 1.0)
                ],
            ))
            .unwrap(),
            vec![
                d_v_double("2023-01-01", 0.25),
                d_v_double("2023-01-02", 4.0)
            ],
        );
        // empty inputs
        assert_eq!(
            AddBurnAndTips::function((
                Vec::<TimespanValue<NaiveDate, f64>>::new(),
                Vec::<TimespanValue<NaiveDate, f64>>::new(),
            ))
            .unwrap(),
            vec![],
        );
    }

    // Expected values are (burn delta + fevm tips) over the Filecoin fixture
    // layer:
    // - `2022-11-09`: the first day of the chart — `Delta` has no prior row,
    //   so the whole starting balance counts as the day's burn; no counted
    //   tips (hazard block);
    // - `2022-11-11`: tips-only day (no f099 row — balance carried over);
    // - `2022-12-15`: genuine no-data day, asserted **by absence** — unfilled
    //   reads omit gap days; the filled `0` is asserted at the API level;
    // - `2023-03-01`: burn-only day (f099 row, no counted tips).
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_filecoin_new_chain_fees() {
        simple_test_chart_filecoin::<FilecoinNewChainFees>(
            "update_filecoin_new_chain_fees",
            vec![
                ("2022-11-09", "30000000"),
                ("2022-11-10", "1000.0005840074068"),
                ("2022-11-11", "0.001193214813588"),
                ("2022-11-12", "2500.000789548147"),
                ("2022-12-01", "6500.0008839185175"),
                ("2023-01-01", "10000.000021492593"),
                ("2023-02-01", "15000.001051166666"),
                ("2023-03-01", "15000"),
            ],
        )
        .await;
    }

    // the implementation is generic over resolutions,
    // therefore other res should also work fine
    // (tests are becoming excruciatingly slow)
}
