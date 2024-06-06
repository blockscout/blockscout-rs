use crate::data_source::kinds::updateable_chart::clone::CloneChartWrapper;

pub mod _inner {
    use crate::{
        data_source::{
            kinds::{
                remote::{RemoteSource, RemoteSourceWrapper},
                updateable_chart::clone::CloneChart,
            },
            UpdateContext,
        },
        missing_date::fit_into_range,
        tests::types::Get,
        Chart, DateValueString, MissingDatePolicy, Named, UpdateError,
    };

    use chrono::{Duration, NaiveDate};
    use entity::sea_orm_active_enums::ChartType;
    use rand::{distributions::uniform::SampleUniform, rngs::StdRng, Rng, SeedableRng};
    use sea_orm::{prelude::*, Statement};
    use std::{
        marker::PhantomData,
        ops::{Range, RangeInclusive},
        str::FromStr,
    };

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
    pub struct MockLineRemote<ValueRange, Value, Policy>(PhantomData<(ValueRange, Value, Policy)>);

    impl<ValueRange, Value, Policy> RemoteSource for MockLineRemote<ValueRange, Value, Policy>
    where
        ValueRange: Get<Range<Value>>,
        Value: SampleUniform + PartialOrd + Clone + ToString + Send + Sync + 'static,
        Policy: Get<MissingDatePolicy>,
    {
        type Point = DateValueString;
        fn get_query(_range: Option<RangeInclusive<DateTimeUtc>>) -> Statement {
            unreachable!()
        }

        async fn query_data(
            cx: &UpdateContext<'_>,
            range: Option<RangeInclusive<DateTimeUtc>>,
        ) -> Result<Vec<Self::Point>, UpdateError> {
            let date_range_start = if let Some(r) = range.clone() {
                r.start().clone()
            } else {
                DateTimeUtc::MIN_UTC
            };
            let mut date_range_end = cx.time;
            if let Some(r) = range {
                date_range_end = date_range_end.min(r.end().clone())
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

    pub struct MockLineInner<ValueRange, Value, Policy>(PhantomData<(ValueRange, Value, Policy)>);

    impl<ValueRange, Value, Policy> Named for MockLineInner<ValueRange, Value, Policy> {
        const NAME: &'static str = "mockLine";
    }

    impl<ValueRange, Value, Policy> Chart for MockLineInner<ValueRange, Value, Policy>
    where
        ValueRange: Sync,
        Value: Sync,
        Policy: Sync,
    {
        fn chart_type() -> ChartType {
            ChartType::Line
        }
    }

    impl<ValueRange, Value, Policy> CloneChart for MockLineInner<ValueRange, Value, Policy>
    where
        ValueRange: Get<Range<Value>> + Sync,
        Value: SampleUniform + PartialOrd + Clone + ToString + Send + Sync + 'static,
        Policy: Get<MissingDatePolicy> + Sync,
    {
        type Dependency = RemoteSourceWrapper<MockLineRemote<ValueRange, Value, Policy>>;
    }
}

pub type MockLine<ValueRange, Value, Policy> =
    CloneChartWrapper<_inner::MockLineInner<ValueRange, Value, Policy>>;
