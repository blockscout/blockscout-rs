//! Similar to `ChartPartialUpdater`, but performs multiple updates for each time period
//! ([`step_duration`]).
//!
//! Useful when each period is independent and this division can help with performance.
//! I.e. if updating a large interval at once (e.g. like `ChartPartialUpdater` does in
//! `force_full` or initial updates) is too expensive.

use super::{
    common_operations::{get_min_block_blockscout, get_min_date_blockscout, get_nth_last_row},
    ChartUpdater,
};
use crate::{
    charts::{
        db_interaction::{types::DateValue, write::insert_data_many},
        find_chart,
    },
    data_source::types::{UpdateContext, UpdateParameters},
    metrics, UpdateError,
};
use chrono::{Duration, NaiveDate};
use sea_orm::{DatabaseConnection, FromQueryResult, Statement, TransactionTrait};
use std::time::Instant;

pub trait ChartBatchUpdater: ChartUpdater {
    // todo: leave only query
    fn get_query(from: NaiveDate, to: NaiveDate) -> Statement;

    async fn update_with_values(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let cx = &cx.user_context;
        let UpdateParameters {
            db,
            blockscout,
            current_time,
            force_full,
        } = *cx;
        let chart_id = find_chart(db, Self::name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(Self::name().into()))?;
        let min_blockscout_block = get_min_block_blockscout(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let offset = Some(Self::approximate_trailing_points());
        let last_updated_row =
            get_nth_last_row::<Self>(chart_id, min_blockscout_block, db, force_full, offset)
                .await?;

        let _timer = metrics::CHART_FETCH_NEW_DATA_TIME
            .with_label_values(&[Self::name()])
            .start_timer();

        tracing::info!(last_updated_row =? last_updated_row, "start batch update");
        Self::batch_update(
            db,
            blockscout,
            last_updated_row,
            current_time.date_naive(),
            chart_id,
            min_blockscout_block,
        )
        .await?;
        // todo: fix
        Ok(vec![])
    }

    async fn batch_update(
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        update_from_row: Option<DateValue>,
        today: NaiveDate,
        chart_id: i32,
        min_blockscout_block: i64,
    ) -> Result<(), UpdateError> {
        let txn = blockscout
            .begin()
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let first_date = match update_from_row {
            Some(row) => row.date,
            None => get_min_date_blockscout(&txn)
                .await
                .map(|time| time.date())
                .map_err(UpdateError::BlockscoutDB)?,
        };

        let steps = generate_date_ranges(first_date, today, Duration::days(30));
        let n = steps.len();

        for (i, (from, to)) in steps.into_iter().enumerate() {
            tracing::info!(from =? from, to =? to , "run {}/{} step of batch update", i + 1, n);
            let query = Self::get_query(from, to);
            let now = Instant::now();
            let values = DateValue::find_by_statement(query)
                .all(&txn)
                .await
                .map_err(UpdateError::BlockscoutDB)?
                .into_iter()
                .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));
            let elapsed = now.elapsed();
            let found = values.len();
            tracing::info!(found =? found, elapsed =? elapsed, "{}/{} step of batch done", i + 1, n);
            insert_data_many(db, values)
                .await
                .map_err(UpdateError::StatsDB)?;
        }
        Ok(())
    }
}

pub fn generate_date_ranges(
    start: NaiveDate,
    end: NaiveDate,
    step: Duration,
) -> Vec<(NaiveDate, NaiveDate)> {
    let mut date_range = Vec::new();
    let mut current_date = start;

    while current_date < end {
        let next_date = current_date + step;
        date_range.push((current_date, next_date));
        current_date = next_date;
    }

    date_range
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::from_str(s).expect("cannot parse date")
    }

    #[test]
    fn test_generate_date_ranges() {
        for ((from, to), expected) in [
            (
                (d("2022-01-01"), d("2022-03-14")),
                vec![
                    (d("2022-01-01"), d("2022-01-31")),
                    (d("2022-01-31"), d("2022-03-02")),
                    (d("2022-03-02"), d("2022-04-01")),
                ],
            ),
            (
                (d("2015-07-20"), d("2015-12-31")),
                vec![
                    (d("2015-07-20"), d("2015-08-19")),
                    (d("2015-08-19"), d("2015-09-18")),
                    (d("2015-09-18"), d("2015-10-18")),
                    (d("2015-10-18"), d("2015-11-17")),
                    (d("2015-11-17"), d("2015-12-17")),
                    (d("2015-12-17"), d("2016-01-16")),
                ],
            ),
            ((d("2015-07-20"), d("2015-07-20")), vec![]),
            (
                (d("2015-07-20"), d("2015-07-21")),
                vec![(d("2015-07-20"), d("2015-08-19"))],
            ),
        ] {
            let actual = generate_date_ranges(from, to, Duration::days(30));
            assert_eq!(expected, actual);
        }
    }
}
