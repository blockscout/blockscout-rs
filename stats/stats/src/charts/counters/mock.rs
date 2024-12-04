use std::marker::PhantomData;

use crate::{
    data_source::{
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
        types::Get,
        UpdateContext,
    },
    range::UniversalRange,
    types::timespans::DateValue,
    ChartProperties, Named, ChartError,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;

pub struct MockCounterRetrieve<PointDateTime, Value>(PhantomData<(PointDateTime, Value)>)
where
    PointDateTime: Get<Value = DateTime<Utc>>,
    Value: Get<Value = String>;

impl<PointDateTime, Value> RemoteQueryBehaviour for MockCounterRetrieve<PointDateTime, Value>
where
    PointDateTime: Get<Value = DateTime<Utc>>,
    Value: Get<Value = String>,
{
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
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

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "mockCounter".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
}

pub type MockCounter<PointDateTime, Value> = DirectPointLocalDbChartSource<
    RemoteDatabaseSource<MockCounterRetrieve<PointDateTime, Value>>,
    Properties,
>;
