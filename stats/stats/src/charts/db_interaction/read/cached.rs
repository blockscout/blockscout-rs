/// Cached query methods. Can be used on any db, because why not.
/// Intended for heavy queries to blockscout database.
use crate::{
    ChartError,
    data_source::{
        UpdateContext,
        types::{Cacheable, UpdateCache},
    },
    types::TimespanTrait,
};

use sea_orm::{ConnectionTrait, FromQueryResult, Statement};

use super::{find_all_points, find_one_value};

pub async fn find_one_value_cached<Value>(
    cx: &UpdateContext<'_>,
    query: Statement,
) -> Result<Option<Value>, ChartError>
where
    Value: FromQueryResult + Cacheable + Clone,
{
    if let Some(cached) = cx.cache.get::<Value>(&query).await {
        Ok(Some(cached))
    } else {
        let value: Option<Value> = find_one_value(cx, query.clone()).await?;
        if let Some(v) = &value {
            // don't cache `None` value because convenience
            cx.cache.insert(&query, v.clone()).await;
        }
        Ok(value)
    }
}

pub async fn find_all_cached<C, Point>(
    cache: &UpdateCache,
    db: &C,
    query: Statement,
) -> Result<Vec<Point>, ChartError>
where
    C: ConnectionTrait,
    Point: FromQueryResult + TimespanTrait + Clone,
    Point::Timespan: Ord,
    Vec<Point>: Cacheable,
{
    if let Some(cached) = cache.get::<Vec<Point>>(&query).await {
        Ok(cached)
    } else {
        let data = find_all_points(db, query.clone()).await?;
        cache.insert(&query, data.clone()).await;
        Ok(data)
    }
}
