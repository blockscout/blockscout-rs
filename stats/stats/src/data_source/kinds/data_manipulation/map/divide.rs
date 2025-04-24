use std::cmp::Ordering;

use itertools::EitherOrBoth;
use num_traits::AsPrimitive;

use crate::{
    data_processing::zip_same_timespan, data_source::kinds::data_manipulation::map::MapFunction,
    types::TimespanValue, ChartError,
};

use super::Map;

pub struct DivisionFunction;

type DivisionPointInput<Resolution, A, B> =
    (TimespanValue<Resolution, A>, TimespanValue<Resolution, B>);

impl<Resolution, A, B> MapFunction<DivisionPointInput<Resolution, A, B>> for DivisionFunction
where
    Resolution: Send + Ord,
    A: AsPrimitive<f64>,
    B: AsPrimitive<f64>,
{
    type Output = TimespanValue<Resolution, f64>;

    fn function(
        inner_data: DivisionPointInput<Resolution, A, B>,
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

type DivisionVectorInput<Resolution, A, B> = (
    Vec<TimespanValue<Resolution, A>>,
    Vec<TimespanValue<Resolution, B>>,
);

impl<Resolution, A, B> MapFunction<DivisionVectorInput<Resolution, A, B>> for DivisionFunction
where
    Resolution: Send + Ord + std::fmt::Debug,
    A: AsPrimitive<f64>,
    B: AsPrimitive<f64>,
{
    type Output = Vec<TimespanValue<Resolution, f64>>;

    fn function(
        inner_data: DivisionVectorInput<Resolution, A, B>,
    ) -> Result<Self::Output, ChartError> {
        let (dividend, divisor) = inner_data;
        let combined = zip_same_timespan(dividend, divisor);
        Ok(combined
            .into_iter()
            .filter_map(|(timespan, values)| calcuate_average_from_either_or_both(timespan, values))
            .collect())
    }
}

fn calcuate_average_from_either_or_both<Resolution, A, B>(
    timespan: Resolution,
    values: EitherOrBoth<A, B>,
) -> Option<TimespanValue<Resolution, f64>>
where
    A: AsPrimitive<f64>,
    B: AsPrimitive<f64>,
{
    match values {
        itertools::EitherOrBoth::Both(dividend, divisor) => Some(TimespanValue {
            timespan,
            value: calculate_average(dividend, divisor),
        }),
        itertools::EitherOrBoth::Right(_divisor) => Some(TimespanValue {
            timespan,
            value: 0.0,
        }),
        itertools::EitherOrBoth::Left(_dividend) => {
            tracing::warn!("{}", division_by_zero_message());
            None
        }
    }
}

fn calculate_average<A, B>(dividend: A, divisor: B) -> f64
where
    A: AsPrimitive<f64>,
    B: AsPrimitive<f64>,
{
    let divisor: f64 = divisor.as_();
    let dividend: f64 = dividend.as_();
    if divisor > 0.0 {
        dividend / divisor
    } else {
        0.0
    }
}

fn division_by_zero_message() -> String {
    "Encountered zero/absent divisor when calculating the fraction".to_owned()
}

/// Divide points date-by-date.
///
///
pub type MapDivide<Dividend, Divisor> = Map<(Dividend, Divisor), DivisionFunction>;
