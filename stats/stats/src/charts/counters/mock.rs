use _inner::MockCounterInner;

use crate::data_source::kinds::updateable_chart::clone::point::ClonePointChartWrapper;

mod _inner {
    use std::{marker::PhantomData, ops::RangeInclusive};

    use crate::{
        charts::db_interaction::types::DateValue,
        data_source::{
            kinds::{
                remote_db::{QueryBehaviour, RemoteDatabaseSource},
                updateable_chart::clone::point::ClonePointChart,
            },
            UpdateContext,
        },
        tests::types::Get,
        Chart, DateValueString, Named, UpdateError,
    };

    use chrono::{DateTime, Utc};
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::prelude::DateTimeUtc;

    pub struct MockCounterRetrieve<PointDateTime, Value>(PhantomData<(PointDateTime, Value)>)
    where
        PointDateTime: Get<DateTime<Utc>>,
        Value: Get<String>;

    impl<PointDateTime, Value> QueryBehaviour for MockCounterRetrieve<PointDateTime, Value>
    where
        PointDateTime: Get<DateTime<Utc>>,
        Value: Get<String>,
    {
        type Output = DateValueString;

        async fn query_data(
            cx: &UpdateContext<'_>,
            _range: Option<RangeInclusive<DateTimeUtc>>,
        ) -> Result<Self::Output, UpdateError> {
            if cx.time >= PointDateTime::get() {
                Ok(DateValueString::from_parts(
                    PointDateTime::get().date_naive(),
                    Value::get(),
                ))
            } else {
                Ok(DateValueString::from_parts(
                    cx.time.date_naive(),
                    "0".to_string(),
                ))
            }
        }
    }

    pub struct MockCounterInner<PointDateTime: Get<DateTime<Utc>>, Value: Get<String>>(
        PhantomData<(PointDateTime, Value)>,
    );

    impl<PointDateTime: Get<DateTime<Utc>>, Value: Get<String>> Named
        for MockCounterInner<PointDateTime, Value>
    {
        const NAME: &'static str = "mockCounter";
    }

    impl<PointDateTime, Value> Chart for MockCounterInner<PointDateTime, Value>
    where
        PointDateTime: Get<DateTime<Utc>> + Sync,
        Value: Get<String> + Sync,
    {
        fn chart_type() -> ChartType {
            ChartType::Counter
        }
    }

    impl<PointDateTime, Value> ClonePointChart for MockCounterInner<PointDateTime, Value>
    where
        PointDateTime: Get<DateTime<Utc>> + Sync,
        Value: Get<String> + Sync,
    {
        type Dependency = RemoteDatabaseSource<MockCounterRetrieve<PointDateTime, Value>>;
    }
}

pub type MockCounter<PointDateTime, Value> =
    ClonePointChartWrapper<MockCounterInner<PointDateTime, Value>>;
