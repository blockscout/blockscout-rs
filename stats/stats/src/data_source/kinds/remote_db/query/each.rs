use std::{
    fmt::Debug,
    marker::{PhantomData, Send},
    ops::Range,
};

use chrono::{DateTime, Utc};
use sea_orm::{FromQueryResult, Statement, TryGetable};

use crate::{
    data_source::{
        kinds::remote_db::RemoteQueryBehaviour,
        types::{BlockscoutMigrations, UpdateContext},
    },
    types::{Timespan, TimespanValue},
    UpdateError,
};

pub trait StatementFromTimespan {
    fn get_statement(
        point: Range<DateTime<Utc>>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement;
}

#[derive(FromQueryResult, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ValueWrapper<V: TryGetable> {
    value: V,
}

/// Pull each point in range with the provided statement `S`.
///
/// `P` - Type of point to retrieve within query.
/// `DateValue<String>` can be used to avoid parsing the values,
/// but `DateValue<Decimal>` or other types can be useful sometimes.
pub struct PullEachWith<S, Resolution, Value, AllRangeSource>(
    PhantomData<(S, Resolution, Value, AllRangeSource)>,
);

impl<S, Resolution, Value, AllRangeSource> RemoteQueryBehaviour
    for PullEachWith<S, Resolution, Value, AllRangeSource>
where
    S: StatementFromTimespan,
    Resolution: Timespan + PartialEq + Debug + Send,
    Value: Send + TryGetable,
    AllRangeSource: RemoteQueryBehaviour<Output = Range<DateTime<Utc>>>,
{
    type Output = Vec<TimespanValue<Resolution, Value>>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTime<Utc>>>,
    ) -> Result<Vec<TimespanValue<Resolution, Value>>, UpdateError> {
        let query_range = if let Some(r) = range {
            r
        } else {
            AllRangeSource::query_data(cx, None).await?
        };
        let points = split_time_range_into_resolution_points::<Resolution>(query_range);
        let mut collected_data = Vec::with_capacity(points.len());
        for point_range in points {
            let query = S::get_statement(point_range.clone(), &cx.blockscout_applied_migrations);
            let point_value = ValueWrapper::<Value>::find_by_statement(query)
                .one(cx.blockscout)
                .await
                .map_err(UpdateError::BlockscoutDB)?;
            if let Some(ValueWrapper { value }) = point_value {
                let timespan = resolution_from_range(point_range);
                collected_data.push(TimespanValue { timespan, value });
            }
        }
        Ok(collected_data)
    }
}

fn resolution_from_range<R: Timespan + PartialEq + Debug>(range: Range<DateTime<Utc>>) -> R {
    let res = R::from_date(range.start.date_naive());
    let res_verify = R::from_date(range.end.date_naive());
    if res_verify != res {
        tracing::warn!(
            range = ?range,
            "Resolution ranges were constructed incorrectly, will likely lead to wrong data. \
            See `split_time_range_into_resolution_points` for the bug."
        );
        debug_assert_eq!(res_verify, res);
    }
    res
}

fn split_time_range_into_resolution_points<Resolution: Timespan>(
    range: Range<DateTime<Utc>>,
) -> Vec<Range<DateTime<Utc>>> {
    let mut result = vec![];
    let mut start = range.start;
    while start < range.end {
        let current_resolution_end = Resolution::from_date(start.date_naive())
            .into_time_range()
            .end;
        let end = current_resolution_end.min(range.end);
        result.push(start..end);
        start = end;
    }
    result
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;

    use crate::{tests::point_construction::dt, types::timespans::Week};

    use super::*;

    #[test]
    fn split_time_range_into_day_points_works() {
        assert_eq!(
            split_time_range_into_resolution_points::<NaiveDate>(
                dt("2023-02-01T09:00:01").and_utc()..dt("2023-02-01T09:00:01").and_utc()
            ),
            vec![]
        );
        assert_eq!(
            split_time_range_into_resolution_points::<NaiveDate>(
                dt("2023-02-01T09:00:01").and_utc()..dt("2023-02-01T10:00:01").and_utc()
            ),
            vec![dt("2023-02-01T09:00:01").and_utc()..dt("2023-02-01T10:00:01").and_utc()]
        );
        assert_eq!(
            split_time_range_into_resolution_points::<NaiveDate>(
                dt("2023-02-01T09:00:01").and_utc()..dt("2023-02-02T10:00:01").and_utc()
            ),
            vec![
                dt("2023-02-01T09:00:01").and_utc()..dt("2023-02-02T00:00:00").and_utc(),
                dt("2023-02-02T00:00:00").and_utc()..dt("2023-02-02T10:00:01").and_utc(),
            ]
        );
        assert_eq!(
            split_time_range_into_resolution_points::<NaiveDate>(
                dt("2023-02-01T09:00:01").and_utc()..dt("2023-02-02T00:00:00").and_utc()
            ),
            vec![dt("2023-02-01T09:00:01").and_utc()..dt("2023-02-02T00:00:00").and_utc(),]
        );
        assert_eq!(
            split_time_range_into_resolution_points::<NaiveDate>(
                dt("2023-02-02T00:00:00").and_utc()..dt("2023-02-02T10:00:01").and_utc()
            ),
            vec![dt("2023-02-02T00:00:00").and_utc()..dt("2023-02-02T10:00:01").and_utc(),]
        );
    }

    #[test]
    fn split_time_range_into_week_points_works() {
        // months & years should also work (same interface)
        assert_eq!(
            split_time_range_into_resolution_points::<Week>(
                dt("2023-02-01T09:00:01").and_utc()..dt("2023-02-01T09:00:01").and_utc()
            ),
            vec![]
        );
        assert_eq!(
            split_time_range_into_resolution_points::<Week>(
                dt("2023-02-01T09:00:01").and_utc()..dt("2023-02-01T10:00:01").and_utc()
            ),
            vec![dt("2023-02-01T09:00:01").and_utc()..dt("2023-02-01T10:00:01").and_utc()]
        );
        assert_eq!(
            split_time_range_into_resolution_points::<Week>(
                dt("2023-02-01T09:00:01").and_utc()..dt("2023-02-07T10:00:01").and_utc()
            ),
            vec![
                dt("2023-02-01T09:00:01").and_utc()..dt("2023-02-06T00:00:00").and_utc(),
                dt("2023-02-06T00:00:00").and_utc()..dt("2023-02-07T10:00:01").and_utc(),
            ]
        );
        assert_eq!(
            split_time_range_into_resolution_points::<Week>(
                dt("2023-02-01T09:00:01").and_utc()..dt("2023-02-06T00:00:00").and_utc()
            ),
            vec![dt("2023-02-01T09:00:01").and_utc()..dt("2023-02-06T00:00:00").and_utc(),]
        );
        assert_eq!(
            split_time_range_into_resolution_points::<Week>(
                dt("2023-02-06T00:00:00").and_utc()..dt("2023-02-07T10:00:01").and_utc()
            ),
            vec![dt("2023-02-06T00:00:00").and_utc()..dt("2023-02-07T10:00:01").and_utc(),]
        );
    }
}
