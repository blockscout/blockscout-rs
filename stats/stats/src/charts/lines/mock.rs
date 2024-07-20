use crate::{
    data_source::{
        kinds::{
            local_db::{
                parameters::update::batching::parameters::Batch30Days, DirectVecLocalDbChartSource,
            },
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
        types::Get,
        UpdateContext,
    },
    missing_date::fit_into_range,
    types::DateValue,
    ChartProperties, MissingDatePolicy, Named, UpdateError,
};

use chrono::{Duration, NaiveDate};
use entity::sea_orm_active_enums::ChartType;
use rand::{distributions::uniform::SampleUniform, rngs::StdRng, Rng, SeedableRng};
use sea_orm::prelude::*;
use std::{marker::PhantomData, ops::Range};

/// non-inclusive range
fn generate_intervals(mut start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let mut times = vec![];
    while start < end {
        times.push(start);
        start += Duration::days(1);
    }
    times
}

pub fn mocked_lines<T: SampleUniform + PartialOrd + Clone + ToString>(
    dates_range: Range<NaiveDate>,
    values_range: Range<T>,
) -> Vec<DateValue<String>> {
    let mut rng = StdRng::seed_from_u64(222);
    generate_intervals(dates_range.start, dates_range.end)
        .into_iter()
        .map(|timespan| {
            let range = values_range.clone();
            let value = rng.gen_range(range);
            DateValue::<String> {
                timespan,
                value: value.to_string(),
            }
        })
        .collect()
}

/// Mock remote source. Can only return values from `mocked_lines`, but respects query time
/// to not include future data.
pub struct MockLineRetrieve<DateRange, ValueRange, Value, Policy>(
    PhantomData<(DateRange, ValueRange, Value, Policy)>,
);

impl<DateRange, ValueRange, Value, Policy> RemoteQueryBehaviour
    for MockLineRetrieve<DateRange, ValueRange, Value, Policy>
where
    DateRange: Get<Range<NaiveDate>>,
    ValueRange: Get<Range<Value>>,
    Value: SampleUniform + PartialOrd + Clone + ToString + Send + Sync + 'static,
    Policy: Get<MissingDatePolicy>,
{
    type Output = Vec<DateValue<String>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTimeUtc>>,
    ) -> Result<Vec<DateValue<String>>, UpdateError> {
        let date_range_start = if let Some(r) = range.clone() {
            r.start
        } else {
            DateTimeUtc::MIN_UTC
        };
        let mut date_range_end = cx.time;
        if let Some(r) = range {
            date_range_end = date_range_end.min(r.end)
        }
        let full_data = mocked_lines(DateRange::get(), ValueRange::get());
        let data = fit_into_range(
            full_data,
            Some(date_range_start.date_naive()),
            Some(date_range_end.date_naive()),
            Policy::get(),
        );

        Ok(data)
    }
}

pub struct MockLineProperties<Value, Policy>(PhantomData<(Value, Policy)>);

impl<Value, Policy> Named for MockLineProperties<Value, Policy> {
    fn name() -> String {
        "mockLine".into()
    }
}

impl<Value, Policy> ChartProperties for MockLineProperties<Value, Policy>
where
    Value: Sync,
    Policy: Sync,
{
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type MockRetrieve<DateRange, ValueRange, Value, Policy> =
    RemoteDatabaseSource<MockLineRetrieve<DateRange, ValueRange, Value, Policy>>;

pub type MockLine<DateRange, ValueRange, Value, Policy> = DirectVecLocalDbChartSource<
    MockRetrieve<DateRange, ValueRange, Value, Policy>,
    Batch30Days,
    MockLineProperties<Value, Policy>,
>;
