use std::cmp::Ordering;

use itertools::EitherOrBoth;

use crate::{
    data_processing::zip_same_timespan, data_source::kinds::data_manipulation::map::MapFunction,
    types::TimespanValue, ChartError,
};

use super::Map;

/// Compute average value from total and count
pub struct AverageFunction;

impl<Resolution>
    MapFunction<(
        TimespanValue<Resolution, i64>,
        TimespanValue<Resolution, i64>,
    )> for AverageFunction
where
    Resolution: Send + Ord,
{
    type Output = TimespanValue<Resolution, f64>;

    fn function(
        inner_data: (
            TimespanValue<Resolution, i64>,
            TimespanValue<Resolution, i64>,
        ),
    ) -> Result<Self::Output, ChartError> {
        let (timespan, values) = match inner_data.0.timespan.cmp(&inner_data.1.timespan) {
            Ordering::Equal => (
                inner_data.0.timespan,
                EitherOrBoth::Both(inner_data.0.value, inner_data.1.value),
            ),
            Ordering::Less => (
                inner_data.0.timespan,
                EitherOrBoth::Left(inner_data.0.value),
            ),
            Ordering::Greater => (
                inner_data.1.timespan,
                EitherOrBoth::Right(inner_data.1.value),
            ),
        };
        calcuate_average_from_either_or_both(timespan, values)
            .ok_or(ChartError::Internal(division_by_zero_message()))
    }
}

impl<Resolution>
    MapFunction<(
        Vec<TimespanValue<Resolution, i64>>,
        Vec<TimespanValue<Resolution, i64>>,
    )> for AverageFunction
where
    Resolution: Send + Ord + std::fmt::Debug,
{
    type Output = Vec<TimespanValue<Resolution, f64>>;

    fn function(
        inner_data: (
            Vec<TimespanValue<Resolution, i64>>,
            Vec<TimespanValue<Resolution, i64>>,
        ),
    ) -> Result<Self::Output, ChartError> {
        let (totals, counts) = inner_data;
        dbg!(&totals);
        dbg!(&counts);
        let combined = zip_same_timespan(totals, counts);
        Ok(combined
            .into_iter()
            .filter_map(|(timespan, values)| calcuate_average_from_either_or_both(timespan, values))
            .collect())
    }
}

fn calcuate_average_from_either_or_both<Resolution>(
    timespan: Resolution,
    values: EitherOrBoth<i64, i64>,
) -> Option<TimespanValue<Resolution, f64>> {
    dbg!(&values);
    match values {
        itertools::EitherOrBoth::Both(total, count) => Some(TimespanValue {
            timespan,
            value: calculate_average(total, count),
        }),
        itertools::EitherOrBoth::Right(_count) => Some(TimespanValue {
            timespan,
            value: 0.0,
        }),
        itertools::EitherOrBoth::Left(_total) => {
            tracing::warn!("{}", division_by_zero_message());
            None
        }
    }
}

fn calculate_average(total: i64, count: i64) -> f64 {
    if count > 0 {
        total as f64 / count as f64
    } else {
        0.0
    }
}

fn division_by_zero_message() -> String {
    "No items count to calculate average".to_owned()
}

/// Calculate average value from total amount for all items
/// and number of items per period
pub type MapCalculateAverage<Total, Count> = Map<(Total, Count), AverageFunction>;
