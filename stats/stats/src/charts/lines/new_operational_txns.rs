use crate::{
    data_processing::zip_same_timespan,
    data_source::kinds::{
        data_manipulation::{
            map::{Map, MapFunction, MapParseTo, MapToString},
            resolutions::sum::SumLowerResolution,
        },
        local_db::{
            parameters::update::batching::parameters::{
                Batch30Weeks, Batch30Years, Batch36Months, BatchMaxDays,
            },
            DirectVecLocalDbChartSource,
        },
    },
    define_and_impl_resolution_properties,
    types::{
        timespans::{Month, Week, Year},
        Timespan, TimespanValue,
    },
    ChartProperties, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use itertools::Itertools;

use super::{new_blocks::NewBlocksInt, NewTxnsInt};

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

pub struct Calculate;

type Input<Resolution> = (
    // newBlocks
    Vec<TimespanValue<Resolution, i64>>,
    // newTxns
    Vec<TimespanValue<Resolution, i64>>,
);

impl<Resolution> MapFunction<Input<Resolution>> for Calculate
where
    Resolution: Timespan + Send + Ord,
{
    type Output = Vec<TimespanValue<Resolution, String>>;

    fn function(inner_data: Input<Resolution>) -> Result<Self::Output, crate::UpdateError> {
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

pub type NewOperationalTxns = DirectVecLocalDbChartSource<
    Map<(NewBlocksInt, NewTxnsInt), Calculate>,
    BatchMaxDays,
    Properties,
>;
pub type NewOperationalTxnsInt = MapParseTo<NewOperationalTxns, i64>;
pub type NewOperationalTxnsWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewOperationalTxnsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewOperationalTxnsMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewOperationalTxnsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewOperationalTxnsMonthlyInt = MapParseTo<NewOperationalTxnsMonthly, i64>;
pub type NewOperationalTxnsYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewOperationalTxnsMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_operational_txns() {
        simple_test_chart::<NewOperationalTxns>(
            "update_new_operational_txns",
            vec![
                ("2022-11-09", "4"),
                ("2022-11-10", "9"),
                ("2022-11-11", "10"),
                ("2022-11-12", "4"),
                ("2022-12-01", "4"),
                ("2023-01-01", "0"),
                ("2023-02-01", "3"),
                ("2023-03-01", "0"),
            ],
        )
        .await;
    }

    // the implementation is generic over resolutions,
    // therefore other res should also work fine
    // (tests are becoming excruciatingly slow)
}
