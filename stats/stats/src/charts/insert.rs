use chrono::NaiveDate;
use entity::chart_data;
use sea_orm::{prelude::*, sea_query, ConnectionTrait, FromQueryResult, Set};

#[derive(FromQueryResult)]
pub struct DateValueInt {
    pub date: NaiveDate,
    pub value: i64,
}

impl From<DateValueInt> for DateValue {
    fn from(double: DateValueInt) -> Self {
        Self {
            date: double.date,
            value: double.value.to_string(),
        }
    }
}

#[derive(FromQueryResult)]
pub struct DateValueDouble {
    pub date: NaiveDate,
    pub value: f64,
}

impl From<DateValueDouble> for DateValue {
    fn from(double: DateValueDouble) -> Self {
        Self {
            date: double.date,
            value: double.value.to_string(),
        }
    }
}

#[derive(FromQueryResult)]
pub struct DateValue {
    pub date: NaiveDate,
    pub value: String,
}

impl DateValue {
    pub fn active_model(
        &self,
        chart_id: i32,
        min_blockscout_block: Option<i64>,
    ) -> chart_data::ActiveModel {
        chart_data::ActiveModel {
            id: Default::default(),
            chart_id: Set(chart_id),
            date: Set(self.date),
            value: Set(self.value.clone()),
            created_at: Default::default(),
            min_blockscout_block: Set(min_blockscout_block),
        }
    }
}

pub async fn insert_data_many<C, D>(db: &C, data: D) -> Result<(), DbErr>
where
    C: ConnectionTrait,
    D: IntoIterator<Item = chart_data::ActiveModel> + Send + Sync,
{
    chart_data::Entity::insert_many(data)
        .on_conflict(
            sea_query::OnConflict::columns([chart_data::Column::ChartId, chart_data::Column::Date])
                .update_column(chart_data::Column::Value)
                .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}
