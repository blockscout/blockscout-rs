use std::{marker::PhantomData, ops::Range};

use crate::{
    data_source::{
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{QueryBehaviour, RemoteDatabaseSource},
        },
        types::Get,
        UpdateContext,
    },
    types::TimespanValue,
    ChartProperties, DateValueString, Named, UpdateError,
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
        _range: Option<Range<DateTimeUtc>>,
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

pub struct MockCounterProperties<PointDateTime: Get<DateTime<Utc>>, Value: Get<String>>(
    PhantomData<(PointDateTime, Value)>,
);

impl<PointDateTime: Get<DateTime<Utc>>, Value: Get<String>> Named
    for MockCounterProperties<PointDateTime, Value>
{
    const NAME: &'static str = "mockCounter";
}

impl<PointDateTime, Value> ChartProperties for MockCounterProperties<PointDateTime, Value>
where
    PointDateTime: Get<DateTime<Utc>> + Sync,
    Value: Get<String> + Sync,
{
    fn chart_type() -> ChartType {
        ChartType::Counter
    }
}

pub type MockCounter<PointDateTime, Value> = DirectPointLocalDbChartSource<
    RemoteDatabaseSource<MockCounterRetrieve<PointDateTime, Value>>,
    MockCounterProperties<PointDateTime, Value>,
>;
