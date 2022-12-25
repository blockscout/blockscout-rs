use crate::counters_list;
use chrono::{Duration, NaiveDate};
use entity::{
    chart_data_int, charts,
    sea_orm_active_enums::{ChartType, ChartValueType},
};
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, Set};
use std::str::FromStr;

fn generate_intervals(mut start: NaiveDate) -> Vec<NaiveDate> {
    let now = chrono::offset::Utc::now().naive_utc().date();
    let mut times = vec![];
    while start < now {
        times.push(start);
        start += Duration::days(1);
    }
    times
}

pub async fn fill_mock_data(db: &DatabaseConnection) -> Result<(), DbErr> {
    chart_data_int::Entity::delete_many().exec(db).await?;
    charts::Entity::delete_many().exec(db).await?;

    let total_blocks_id = charts::Entity::insert(charts::ActiveModel {
        name: Set(counters_list::TOTAL_BLOCKS.to_string()),
        chart_type: Set(ChartType::Counter),
        value_type: Set(ChartValueType::Int),
        ..Default::default()
    })
    .exec(db)
    .await?;

    let new_blocks_id = charts::Entity::insert(charts::ActiveModel {
        name: Set("newBlocksPerDay".into()),
        chart_type: Set(ChartType::Line),
        value_type: Set(ChartValueType::Int),
        ..Default::default()
    })
    .exec(db)
    .await?;

    chart_data_int::Entity::insert(chart_data_int::ActiveModel {
        chart_id: Set(total_blocks_id.last_insert_id),
        date: Set(chrono::offset::Local::now().naive_utc().date()),
        value: Set(16075890),
        ..Default::default()
    })
    .exec(db)
    .await?;

    chart_data_int::Entity::insert_many(
        generate_intervals(NaiveDate::from_str("2022-01-01").unwrap())
            .into_iter()
            .enumerate()
            .map(|(i, date)| chart_data_int::ActiveModel {
                chart_id: Set(new_blocks_id.last_insert_id),
                date: Set(date),
                value: Set(100 + ((i as i64 * 1103515245 + 12345) % 100)),
                ..Default::default()
            }),
    )
    .exec(db)
    .await?;

    Ok(())
}
