/// Operational transactions - metrics originally designed for Arbitrum.
/// They represent number of transactions excluding a one-per-block system transaction.
///
/// Because of the strict definition, it is simply calculated as a difference
/// between number of transactions and number of blocks.
use std::fmt::Debug;

use crate::{
    ChartProperties, MissingDatePolicy, Named,
    data_processing::zip_same_timespan,
    data_source::kinds::{
        data_manipulation::{
            map::{Map, MapFunction, MapParseTo, MapToString, StripExt},
            resolutions::sum::SumLowerResolution,
        },
        local_db::{
            DirectVecLocalDbChartSource,
            parameters::update::batching::parameters::{
                Batch30Weeks, Batch30Years, Batch36Months, BatchMaxDays,
            },
        },
    },
    define_and_impl_resolution_properties,
    lines::{NewBlocksInt, NewTxnsInt},
    types::{
        Timespan, TimespanValue,
        timespans::{Month, Week, Year},
    },
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use itertools::Itertools;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newOperationalTxns".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }

    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillZero
    }
}

pub struct ArbitrumCalculateOperationalTxnsVec;

type Input<Resolution> = (
    // newBlocks
    Vec<TimespanValue<Resolution, i64>>,
    // newTxns
    Vec<TimespanValue<Resolution, i64>>,
);

impl<Resolution> MapFunction<Input<Resolution>> for ArbitrumCalculateOperationalTxnsVec
where
    Resolution: Timespan + Send + Ord + Debug,
{
    type Output = Vec<TimespanValue<Resolution, String>>;

    fn function(inner_data: Input<Resolution>) -> Result<Self::Output, crate::ChartError> {
        let (blocks_data, txns_data) = inner_data;
        let combined = zip_same_timespan(blocks_data, txns_data);
        let data = combined
            .into_iter()
            .map(|(timespan, data)| {
                // both new blocks & new txns treat missing values as zero
                let (blocks, txns) = data.or(0, 0);
                TimespanValue {
                    timespan,
                    value: txns.saturating_sub(blocks).to_string(),
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

pub type ArbitrumNewOperationalTxns = DirectVecLocalDbChartSource<
    Map<(NewBlocksInt, NewTxnsInt), ArbitrumCalculateOperationalTxnsVec>,
    BatchMaxDays,
    Properties,
>;
pub type ArbitrumNewOperationalTxnsInt = MapParseTo<StripExt<ArbitrumNewOperationalTxns>, i64>;
pub type ArbitrumNewOperationalTxnsWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<ArbitrumNewOperationalTxnsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type ArbitrumNewOperationalTxnsMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<ArbitrumNewOperationalTxnsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type ArbitrumNewOperationalTxnsMonthlyInt =
    MapParseTo<StripExt<ArbitrumNewOperationalTxnsMonthly>, i64>;
pub type ArbitrumNewOperationalTxnsYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<ArbitrumNewOperationalTxnsMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_arbitrum_new_operational_txns() {
        simple_test_chart::<ArbitrumNewOperationalTxns>(
            "update_arbitrum_new_operational_txns",
            vec![
                ("2022-11-09", "5"),
                ("2022-11-10", "11"),
                ("2022-11-11", "12"),
                ("2022-11-12", "5"),
                ("2022-12-01", "5"),
                ("2023-01-01", "0"),
                ("2023-02-01", "4"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }

    // the implementation is generic over resolutions,
    // therefore other res should also work fine
    // (tests are becoming excruciatingly slow)
}
