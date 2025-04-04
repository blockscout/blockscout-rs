/// Cached query methods. Can be used on any db, because why not.
/// Intended for heavy queries to blockscout database.
use crate::{
    data_source::{types::Cacheable, UpdateContext},
    types::TimespanTrait,
    ChartError,
};

use sea_orm::{FromQueryResult, Statement};

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

pub async fn find_all_cached<Point>(
    cx: &UpdateContext<'_>,
    query: Statement,
) -> Result<Vec<Point>, ChartError>
where
    Point: FromQueryResult + TimespanTrait + Clone,
    Point::Timespan: Ord,
    Vec<Point>: Cacheable,
{
    if let Some(cached) = cx.cache.get::<Vec<Point>>(&query).await {
        Ok(cached)
    } else {
        let data = find_all_points(cx, query.clone()).await?;
        cx.cache.insert(&query, data.clone()).await;
        Ok(data)
    }
}
