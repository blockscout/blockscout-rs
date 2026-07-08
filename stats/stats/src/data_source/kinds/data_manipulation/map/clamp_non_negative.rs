// SPDX-License-Identifier: LicenseRef-Blockscout

use std::fmt::Debug;

use num_traits::Zero;

use crate::{ChartError, types::TimespanValue};

use super::{Map, MapFunction};

pub struct ClampNonNegativeFunction;

fn clamp_point<Resolution, Value>(
    point: TimespanValue<Resolution, Value>,
) -> TimespanValue<Resolution, Value>
where
    Resolution: Debug,
    Value: Zero + PartialOrd + Debug,
{
    if point.value < Value::zero() {
        tracing::warn!(
            timespan = ?point.timespan,
            value = ?point.value,
            "negative value clamped to zero; the source is expected to be \
            non-negative, so this can only be a data artifact"
        );
        TimespanValue {
            timespan: point.timespan,
            value: Value::zero(),
        }
    } else {
        point
    }
}

impl<Resolution, Value> MapFunction<Vec<TimespanValue<Resolution, Value>>>
    for ClampNonNegativeFunction
where
    Resolution: Send + Debug,
    Value: Zero + PartialOrd + Send + Debug,
{
    type Output = Vec<TimespanValue<Resolution, Value>>;
    fn function(
        inner_data: Vec<TimespanValue<Resolution, Value>>,
    ) -> Result<Self::Output, ChartError> {
        Ok(inner_data.into_iter().map(clamp_point).collect())
    }
}

impl<Resolution, Value> MapFunction<TimespanValue<Resolution, Value>> for ClampNonNegativeFunction
where
    Resolution: Send + Debug,
    Value: Zero + PartialOrd + Send + Debug,
{
    type Output = TimespanValue<Resolution, Value>;
    fn function(inner_data: TimespanValue<Resolution, Value>) -> Result<Self::Output, ChartError> {
        Ok(clamp_point(inner_data))
    }
}

/// Floor the values of `D` at zero: values below zero are replaced with
/// zero, everything else passes through unchanged.
///
/// Useful when a series is non-negative by construction (e.g. a [`Delta`]
/// over a balance that only ever grows on-chain) and a negative value can
/// only be a data artifact that must not reach the chart.
///
/// [`Delta`]: crate::data_source::kinds::data_manipulation::delta::Delta
pub type ClampNonNegative<D> = Map<D, ClampNonNegativeFunction>;

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::tests::point_construction::d_v_double;

    #[test]
    fn clamps_negatives_and_keeps_the_rest() {
        let input = vec![
            d_v_double("2024-01-01", 1.5),
            d_v_double("2024-01-02", 0.0),
            d_v_double("2024-01-03", -2.5),
            d_v_double("2024-01-04", 3.0),
        ];
        let expected = vec![
            d_v_double("2024-01-01", 1.5),
            d_v_double("2024-01-02", 0.0),
            d_v_double("2024-01-03", 0.0),
            d_v_double("2024-01-04", 3.0),
        ];
        assert_eq!(ClampNonNegativeFunction::function(input).unwrap(), expected);
    }

    #[test]
    fn clamps_a_single_point() {
        assert_eq!(
            ClampNonNegativeFunction::function(d_v_double("2024-01-03", -2.5)).unwrap(),
            d_v_double("2024-01-03", 0.0)
        );
        assert_eq!(
            ClampNonNegativeFunction::function(d_v_double("2024-01-04", 3.0)).unwrap(),
            d_v_double("2024-01-04", 3.0)
        );
    }
}
