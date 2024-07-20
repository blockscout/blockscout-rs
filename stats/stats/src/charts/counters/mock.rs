use std::{marker::PhantomData, ops::Range};

use crate::{
    data_source::{
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
        types::Get,
        UpdateContext,
    },
    types::DateValue,
    ChartProperties, Named, UpdateError,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::DateTimeUtc;

pub struct MockCounterRetrieve<PointDateTime, Value>(PhantomData<(PointDateTime, Value)>)
where
    PointDateTime: Get<DateTime<Utc>>,
    Value: Get<String>;

impl<PointDateTime, Value> RemoteQueryBehaviour for MockCounterRetrieve<PointDateTime, Value>
where
    PointDateTime: Get<DateTime<Utc>>,
    Value: Get<String>,
{
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: Option<Range<DateTimeUtc>>,
    ) -> Result<Self::Output, UpdateError> {
        if cx.time >= PointDateTime::get() {
            Ok(DateValue::<String> {
                timespan: PointDateTime::get().date_naive(),
                value: Value::get(),
            })
        } else {
            Ok(DateValue::<String> {
                timespan: cx.time.date_naive(),
                value: "0".to_string(),
            })
        }
    }
}

pub struct MockCounterProperties<PointDateTime: Get<DateTime<Utc>>, Value: Get<String>>(
    PhantomData<(PointDateTime, Value)>,
);

impl<PointDateTime: Get<DateTime<Utc>>, Value: Get<String>> Named
    for MockCounterProperties<PointDateTime, Value>
{
    fn name() -> String {
        "mockCounter".into()
    }
}

impl<PointDateTime, Value> ChartProperties for MockCounterProperties<PointDateTime, Value>
where
    PointDateTime: Get<DateTime<Utc>> + Sync,
    Value: Get<String> + Sync,
{
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
}

pub type MockCounter<PointDateTime, Value> = DirectPointLocalDbChartSource<
    RemoteDatabaseSource<MockCounterRetrieve<PointDateTime, Value>>,
    MockCounterProperties<PointDateTime, Value>,
>;
