use std::ops::Range;

use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, FromQueryResult, QueryFilter, QueryOrder,
    QuerySelect, sea_query::Func,
};
use zetachain_cctx_entity::{sea_orm_active_enums::Kind, watermark};

use crate::{
    ChartError,
    data_source::{UpdateContext, kinds::remote_db::RemoteQueryBehaviour},
    range::UniversalRange,
};

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

#[derive(FromQueryResult)]
struct MinTimestamp {
    timestamp: NaiveDateTime,
}

pub async fn get_min_timestamp_zetachain_cctx<C>(db: &C) -> Result<NaiveDateTime, DbErr>
where
    C: ConnectionTrait,
{
    let min_date = zetachain_cctx_entity::cctx_status::Entity::find()
        .select_only()
        .expr_as(
            Func::cust("to_timestamp")
                .arg(zetachain_cctx_entity::cctx_status::Column::CreatedTimestamp.into_expr()),
            "timestamp",
        )
        .order_by_asc(zetachain_cctx_entity::cctx_status::Column::CreatedTimestamp)
        .into_model::<MinTimestamp>()
        .one(db)
        .await?;

    min_date.map(|r| r.timestamp).ok_or_else(|| {
        DbErr::RecordNotFound("no crosschain txns found in zetachain cctx database".into())
    })
}

pub struct QueryAllCctxTimetsampRange;

impl RemoteQueryBehaviour for QueryAllCctxTimetsampRange {
    type Output = Range<DateTime<Utc>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let Some(db) = cx.second_indexer_db else {
            return Err(ChartError::Internal("Cannot query all zetachain cctx timestamp range: zetachain indexer DB is not connected".to_string()));
        };
        let start_timestamp = get_min_timestamp_zetachain_cctx(db)
            .await
            .map_err(ChartError::IndexerDB)?
            .and_utc();
        Ok(start_timestamp..cx.time)
    }
}
