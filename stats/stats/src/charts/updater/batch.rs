use super::{get_last_row, get_min_block_blockscout, get_min_date_blockscout};
use crate::{
    charts::{find_chart, insert::insert_data_many},
    metrics, Chart, DateValue, UpdateError,
};
use async_trait::async_trait;
use chrono::{Duration, NaiveDate, Utc};
use sea_orm::{DatabaseConnection, FromQueryResult, Statement, TransactionTrait};
use std::time::Instant;

#[async_trait]
pub trait ChartBatchUpdater: Chart {
    fn get_query(&self, from: NaiveDate, to: NaiveDate) -> Statement;
    fn step_duration(&self) -> chrono::Duration {
        chrono::Duration::days(30)
    }

    async fn update_with_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = find_chart(db, self.name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let min_blockscout_block = get_min_block_blockscout(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let last_row = get_last_row(self, chart_id, min_blockscout_block, db, force_full).await?;

        let _timer = metrics::CHART_FETCH_NEW_DATA_TIME
            .with_label_values(&[self.name()])
            .start_timer();
        tracing::info!(last_row =? last_row, "start batch update");
        self.batch_update(db, blockscout, last_row, chart_id, min_blockscout_block)
            .await
    }

    async fn batch_update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        last_row: Option<DateValue>,
        chart_id: i32,
        min_blockscout_block: i64,
    ) -> Result<(), UpdateError> {
        let txn = blockscout
            .begin()
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let first_date = match last_row {
            Some(last_row) => last_row.date,
            None => get_min_date_blockscout(&txn)
                .await
                .map(|time| time.date())
                .map_err(UpdateError::BlockscoutDB)?,
        };
        let last_date = Utc::now().date_naive();

        let steps = generate_date_ranges(first_date, last_date, self.step_duration());
        let n = steps.len();

        for (i, (from, to)) in steps.into_iter().enumerate() {
            tracing::info!(from =? from, to =? to , "run {}/{} step of batch update", i + 1, n);
            let query = self.get_query(from, to);
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

fn generate_date_ranges(
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
        ] {
            let actual = generate_date_ranges(from, to, Duration::days(30));
            assert_eq!(expected, actual);
        }
    }
}
