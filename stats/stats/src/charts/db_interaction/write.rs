use chrono::{DateTime, Offset, TimeZone};
use entity::{chart_data, charts, sea_orm_active_enums::ChartType};
use sea_orm::{prelude::*, sea_query, ConnectionTrait, Set, Unchanged};

use super::read::find_chart;

pub async fn create_chart<Tz: TimeZone>(
    db: &DatabaseConnection,
    name: String,
    chart_type: ChartType,
    creation_time: &DateTime<Tz>,
) -> Result<(), DbErr> {
    let id = find_chart(db, &name).await?;
    if id.is_some() {
        return Ok(());
    }
    charts::Entity::insert(charts::ActiveModel {
        name: Set(name),
        chart_type: Set(chart_type),
        created_at: Set(creation_time.with_timezone(&creation_time.offset().fix())),
        ..Default::default()
    })
    .on_conflict(
        sea_query::OnConflict::column(charts::Column::Name)
            .do_nothing()
            .to_owned(),
    )
    .exec(db)
    .await?;
    Ok(())
}

pub async fn insert_data_many<C, D>(db: &C, data: D) -> Result<(), DbErr>
where
    C: ConnectionTrait,
    D: IntoIterator<Item = chart_data::ActiveModel> + Send + Sync,
{
    let mut data = data.into_iter().peekable();
    if data.peek().is_some() {
        chart_data::Entity::insert_many(data)
            .on_conflict(
                sea_query::OnConflict::columns([
                    chart_data::Column::ChartId,
                    chart_data::Column::Date,
                ])
                .update_column(chart_data::Column::Value)
                .to_owned(),
            )
            .exec(db)
            .await?;
    }
    Ok(())
}

pub async fn set_last_updated_at<Tz>(
    chart_id: i32,
    db: &DatabaseConnection,
    at: chrono::DateTime<Tz>,
) -> Result<(), DbErr>
where
    Tz: chrono::TimeZone,
{
    let last_updated_at = at.with_timezone(&chrono::Utc.fix());
    let model = charts::ActiveModel {
        id: Unchanged(chart_id),
        last_updated_at: Set(Some(last_updated_at)),
        ..Default::default()
    };
    charts::Entity::update(model)
        .filter(charts::Column::Id.eq(chart_id))
        .exec(db)
        .await?;
    Ok(())
}
