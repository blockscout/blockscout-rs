use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter};
use zetachain_cctx_entity::{sea_orm_active_enums::Kind, watermark};

/// `None` if no historical watermark is found or the timestamp is not set
pub async fn query_zetachain_cctx_indexed_until<C: ConnectionTrait>(
    db: &C,
) -> Result<Option<DateTime<Utc>>, DbErr> {
    let historical_watermark = watermark::Entity::find()
        .filter(
            sea_orm::Condition::all()
                .add(watermark::Column::Kind.eq(Kind::Historical))
                .add(watermark::Column::Id.eq(1)),
        )
        .one(db)
        .await?;
    let historical_watermark_timestamp = historical_watermark
        .and_then(|w| w.upper_bound_timestamp)
        .map(|t| t.and_utc());
    Ok(historical_watermark_timestamp)
}
