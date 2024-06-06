use _inner::MockCounterInner;

use crate::data_source::kinds::updateable_chart::clone::point::ClonePointChartWrapper;

mod _inner {
    use std::marker::PhantomData;

    use crate::{
        charts::db_interaction::types::DateValue,
        data_source::{
            kinds::{
                remote::point::{RemotePointSource, RemotePointSourceWrapper},
                updateable_chart::clone::point::ClonePointChart,
            },
            UpdateContext,
        },
        tests::types::Get,
        Chart, DateValueString, Named, UpdateError,
    };

    use chrono::{DateTime, Utc};
    use entity::sea_orm_active_enums::ChartType;

    pub struct MockCounterRemote<PointDateTime: Get<DateTime<Utc>>, Value: Get<String>>(
        PhantomData<(PointDateTime, Value)>,
    );

    impl<PointDateTime, Value> RemotePointSource for MockCounterRemote<PointDateTime, Value>
    where
        PointDateTime: Get<DateTime<Utc>>,
        Value: Get<String>,
    {
        type Point = DateValueString;
        fn get_query() -> sea_orm::Statement {
            unreachable!()
        }

        async fn query_data(cx: &UpdateContext<'_>) -> Result<Self::Point, UpdateError> {
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
        type Dependency = RemotePointSourceWrapper<MockCounterRemote<PointDateTime, Value>>;
    }
}

pub type MockCounter<PointDateTime, Value> =
    ClonePointChartWrapper<MockCounterInner<PointDateTime, Value>>;
