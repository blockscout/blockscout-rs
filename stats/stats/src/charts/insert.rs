use chrono::NaiveDate;
use entity::chart_data;
use sea_orm::{prelude::*, sea_query, ConnectionTrait, FromQueryResult, Set};

#[derive(FromQueryResult)]
pub struct DateValueDouble {
    pub date: NaiveDate,
    pub value: f64,
}

#[derive(FromQueryResult)]
pub struct DateValue {
    pub date: NaiveDate,
    pub value: String,
}

impl From<DateValueDouble> for DateValue {
    fn from(double: DateValueDouble) -> Self {
        Self {
            date: double.date,
            value: double.value.to_string(),
        }
    }
}

impl DateValue {
    pub fn active_model(&self, chart_id: i32) -> chart_data::ActiveModel {
        chart_data::ActiveModel {
            id: Default::default(),
            chart_id: Set(chart_id),
            date: Set(self.date),
            value: Set(self.value.clone()),
            created_at: Default::default(),
        }
    }
}

pub async fn insert_data<C: ConnectionTrait>(
    db: &C,
    chart_id: i32,
    value: DateValue,
) -> Result<(), DbErr> {
    insert_data_many(db, std::iter::once(value.active_model(chart_id))).await
}

pub async fn insert_data_many<C, D>(db: &C, data: D) -> Result<(), DbErr>
where
    C: ConnectionTrait,
    D: IntoIterator<Item = chart_data::ActiveModel>,
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
