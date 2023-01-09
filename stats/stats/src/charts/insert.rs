use chrono::NaiveDate;
use entity::{chart_data_double, chart_data_int};
use sea_orm::{prelude::*, sea_query, ConnectionTrait, FromQueryResult, Set};

#[derive(FromQueryResult)]
pub struct IntValueItem {
    pub date: NaiveDate,
    pub value: i64,
}

#[derive(FromQueryResult)]
pub struct DoubleValueItem {
    pub date: NaiveDate,
    pub value: f64,
}

impl IntValueItem {
    pub fn active_model(&self, chart_id: i32) -> chart_data_int::ActiveModel {
        chart_data_int::ActiveModel {
            id: Default::default(),
            chart_id: Set(chart_id),
            date: Set(self.date),
            value: Set(self.value),
            created_at: Default::default(),
        }
    }
}

impl DoubleValueItem {
    pub fn active_model(&self, chart_id: i32) -> chart_data_double::ActiveModel {
        chart_data_double::ActiveModel {
            id: Default::default(),
            chart_id: Set(chart_id),
            date: Set(self.date),
            value: Set(self.value),
            created_at: Default::default(),
        }
    }
}

pub async fn insert_int_data<C: ConnectionTrait>(
    db: &C,
    chart_id: i32,
    value: IntValueItem,
) -> Result<(), DbErr> {
    insert_int_data_many(db, std::iter::once(value.active_model(chart_id))).await
}

pub async fn insert_double_data<C: ConnectionTrait>(
    db: &C,
    chart_id: i32,
    value: DoubleValueItem,
) -> Result<(), DbErr> {
    insert_double_data_many(db, std::iter::once(value.active_model(chart_id))).await
}

pub async fn insert_int_data_many<C, D>(db: &C, data: D) -> Result<(), DbErr>
where
    C: ConnectionTrait,
    D: IntoIterator<Item = chart_data_int::ActiveModel>,
{
    chart_data_int::Entity::insert_many(data)
        .on_conflict(
            sea_query::OnConflict::columns([
                chart_data_int::Column::ChartId,
                chart_data_int::Column::Date,
            ])
            .update_column(chart_data_int::Column::Value)
            .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}

pub async fn insert_double_data_many<C, D>(db: &C, data: D) -> Result<(), DbErr>
where
    C: ConnectionTrait,
    D: IntoIterator<Item = chart_data_double::ActiveModel>,
{
    chart_data_double::Entity::insert_many(data)
        .on_conflict(
            sea_query::OnConflict::columns([
                chart_data_double::Column::ChartId,
                chart_data_double::Column::Date,
            ])
            .update_column(chart_data_double::Column::Value)
            .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}
