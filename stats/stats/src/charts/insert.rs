use chrono::NaiveDate;
use entity::{chart_data_double, chart_data_int};

use sea_orm::{prelude::*, sea_query, ConnectionTrait, Set};

pub async fn insert_int_data<C: ConnectionTrait>(
    db: &C,
    chart_id: i32,
    date: NaiveDate,
    value: i64,
) -> Result<(), DbErr> {
    let data = chart_data_int::ActiveModel {
        id: Default::default(),
        chart_id: Set(chart_id),
        date: Set(date),
        value: Set(value),
        created_at: Default::default(),
    };

    chart_data_int::Entity::insert(data)
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

pub async fn insert_double_data<C: ConnectionTrait>(
    db: &C,
    chart_id: i32,
    date: NaiveDate,
    value: f64,
) -> Result<(), DbErr> {
    let data = chart_data_double::ActiveModel {
        id: Default::default(),
        chart_id: Set(chart_id),
        date: Set(date),
        value: Set(value),
        created_at: Default::default(),
    };

    chart_data_double::Entity::insert(data)
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
