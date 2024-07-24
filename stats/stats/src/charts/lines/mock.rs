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
    types::{DateValue, Timespan, TimespanValue},
    ChartProperties, MissingDatePolicy, Named, UpdateError,
};

use chrono::{DateTime, Duration, NaiveDate, Utc};
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

pub fn mock_trim_lines<T, V>(
    data: Vec<TimespanValue<T, V>>,
    query_time: DateTime<Utc>,
    query_range: Option<Range<DateTimeUtc>>,
    policy: MissingDatePolicy,
) -> Vec<TimespanValue<T, V>>
where
    T: Timespan + Ord + Clone,
    V: Clone,
{
    let date_range_start = query_range
        .clone()
        .map(|r| T::from_date(r.start.date_naive()));
    let mut date_range_end = query_time;
    if let Some(r) = query_range {
        date_range_end = date_range_end.min(r.end)
    }
    fit_into_range(
        data,
        date_range_start,
        Some(T::from_date(date_range_end.date_naive())),
        policy,
    )
}

/// Mock remote source. Can only return values from `mocked_lines`, but respects query time
/// to not include future data.
pub struct PseudoRandomMockLineRetrieve<DateRange, ValueRange, Value, Policy>(
    PhantomData<(DateRange, ValueRange, Value, Policy)>,
);

impl<DateRange, ValueRange, Value, Policy> RemoteQueryBehaviour
    for PseudoRandomMockLineRetrieve<DateRange, ValueRange, Value, Policy>
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
        let full_data = mocked_lines(DateRange::get(), ValueRange::get());
        Ok(mock_trim_lines(full_data, cx.time, range, Policy::get()))
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

pub type PseudoRandomMockRetrieve<DateRange, ValueRange, Value, Policy> =
    RemoteDatabaseSource<PseudoRandomMockLineRetrieve<DateRange, ValueRange, Value, Policy>>;

pub type PseudoRandomMockLine<DateRange, ValueRange, Value, Policy> = DirectVecLocalDbChartSource<
    PseudoRandomMockRetrieve<DateRange, ValueRange, Value, Policy>,
    Batch30Days,
    MockLineProperties<Value, Policy>,
>;

pub struct PredefinedMockSource<Data, Policy, T, V>(PhantomData<(Data, Policy, T, V)>);

impl<Data, Policy, T, V> RemoteQueryBehaviour for PredefinedMockSource<Data, Policy, T, V>
where
    Data: Get<Vec<TimespanValue<T, V>>>,
    Policy: Get<MissingDatePolicy>,
    TimespanValue<T, V>: Send,
    T: Timespan + Ord + Clone,
    V: Clone,
{
    type Output = Vec<TimespanValue<T, V>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTimeUtc>>,
    ) -> Result<Self::Output, UpdateError> {
        Ok(mock_trim_lines(Data::get(), cx.time, range, Policy::get()))
    }
}
