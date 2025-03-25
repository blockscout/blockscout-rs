/// Cached query methods. Can be used on any db, because why not.
///
use crate::{
    data_source::{types::Cacheable, UpdateContext},
    types::TimespanTrait,
    ChartError,
};

use sea_orm::{FromQueryResult, Statement};

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
        let value = Value::find_by_statement(query.clone())
            .one(cx.blockscout)
            .await
            .map_err(ChartError::BlockscoutDB)?;
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
        let find_by_statement = Point::find_by_statement(query.clone());
        let mut data = find_by_statement
            .all(cx.blockscout)
            .await
            .map_err(ChartError::BlockscoutDB)?;
        // can't use sort_*_by_key: https://github.com/rust-lang/rust/issues/34162
        data.sort_unstable_by(|a, b| a.timespan().cmp(b.timespan()));
        cx.cache.insert(&query, data.clone()).await;
        Ok(data)
    }
}
