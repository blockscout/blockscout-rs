use crate::{
    data_source::{
        kinds::{
            local_db::DirectVecLocalDbChartSource,
            remote_db::{QueryBehaviour, RemoteDatabaseSource},
        },
        types::Get,
        UpdateContext,
    },
    missing_date::fit_into_range,
    ChartProperties, DateValueString, MissingDatePolicy, Named, UpdateError,
};

use chrono::{Duration, NaiveDate};
use entity::sea_orm_active_enums::ChartType;
use rand::{distributions::uniform::SampleUniform, rngs::StdRng, Rng, SeedableRng};
use sea_orm::prelude::*;
use std::{marker::PhantomData, ops::Range, str::FromStr};

fn generate_intervals(mut start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let mut times = vec![];
    while start < end {
        times.push(start);
        start += Duration::days(1);
    }
    times
}

pub fn mocked_lines<T: SampleUniform + PartialOrd + Clone + ToString>(
    range: Range<T>,
) -> Vec<DateValueString> {
    let mut rng = StdRng::seed_from_u64(222);
    generate_intervals(
        NaiveDate::from_str("2022-01-01").unwrap(),
        NaiveDate::from_str("2022-04-01").unwrap(),
    )
    .into_iter()
    .map(|date| {
        let range = range.clone();
        let value = rng.gen_range(range);
        DateValueString {
            date,
            value: value.to_string(),
        }
    })
    .collect()
}

/// Mock remote source. Can only return values from `mocked_lines`, but respects query time
/// to not include future data.
pub struct MockLineRetrieve<ValueRange, Value, Policy>(PhantomData<(ValueRange, Value, Policy)>);

impl<ValueRange, Value, Policy> QueryBehaviour for MockLineRetrieve<ValueRange, Value, Policy>
where
    ValueRange: Get<Range<Value>>,
    Value: SampleUniform + PartialOrd + Clone + ToString + Send + Sync + 'static,
    Policy: Get<MissingDatePolicy>,
{
    type Output = Vec<DateValueString>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTimeUtc>>,
    ) -> Result<Vec<DateValueString>, UpdateError> {
        let date_range_start = if let Some(r) = range.clone() {
            r.start
        } else {
            DateTimeUtc::MIN_UTC
        };
        let mut date_range_end = cx.time;
        if let Some(r) = range {
            date_range_end = date_range_end.min(r.end)
        }
        let full_data = mocked_lines(ValueRange::get());
        let data = fit_into_range(
            full_data,
            Some(date_range_start.date_naive()),
            Some(date_range_end.date_naive()),
            Policy::get(),
        );
        Ok(data)
    }
}

pub struct MockLineProperties<ValueRange, Value, Policy>(PhantomData<(ValueRange, Value, Policy)>);

impl<ValueRange, Value, Policy> Named for MockLineProperties<ValueRange, Value, Policy> {
    const NAME: &'static str = "mockLine";
}

impl<ValueRange, Value, Policy> ChartProperties for MockLineProperties<ValueRange, Value, Policy>
where
    ValueRange: Sync,
    Value: Sync,
    Policy: Sync,
{
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type MockRetrieve<ValueRange, Value, Policy> =
    RemoteDatabaseSource<MockLineRetrieve<ValueRange, Value, Policy>>;

pub type MockLine<ValueRange, Value, Policy> = DirectVecLocalDbChartSource<
    MockRetrieve<ValueRange, Value, Policy>,
    MockLineProperties<ValueRange, Value, Policy>,
>;
