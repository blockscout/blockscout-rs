use chrono::Offset;
use entity::charts;
use sea_orm::{prelude::*, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set};

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
        last_updated_at: Set(Some(last_updated_at)),
        ..Default::default()
    };
    charts::Entity::update(model)
        .filter(charts::Column::Id.eq(chart_id))
        .exec(db)
        .await?;
    Ok(())
}
