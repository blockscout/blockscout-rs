use std::{marker::PhantomData, ops::RangeInclusive, time::Instant};

use chrono::{Duration, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DatabaseConnection, TransactionTrait};

use super::UpdateableChart;
use crate::{
    charts::{
        chart::chart_portrait,
        db_interaction::chart_updaters::common_operations::get_min_date_blockscout,
    },
    data_source::{
        source_trait::DataSource,
        types::{UpdateContext, UpdateParameters},
    },
    Chart, DateValue, MissingDatePolicy, UpdateError,
};

pub trait BatchUpdateableChart: Chart {
    type PrimaryDependency: DataSource;
    type SecondaryDependencies: DataSource;

    // todo: TimeDelta
    fn step_duration() -> chrono::Duration {
        chrono::Duration::days(30)
    }

    /// Update chart with data from its dependencies.
    ///
    /// Returns how many records were found
    async fn batch_update_values_step_with(
        db: &DatabaseConnection,
        chart_id: i32,
        update_time: chrono::DateTime<Utc>,
        min_blockscout_block: i64,
        primary_data: <Self::PrimaryDependency as DataSource>::Output,
        secondary_data: <Self::SecondaryDependencies as DataSource>::Output,
    ) -> Result<usize, UpdateError>;
}

pub struct BatchUpdateableChartWrapper<T: BatchUpdateableChart>(PhantomData<T>);

#[portrait::fill(portrait::delegate(T))]
impl<T: BatchUpdateableChart + Chart> Chart for BatchUpdateableChartWrapper<T> {}

/// Perform update utilizing batching
async fn batch_update_values<U>(
    cx: &UpdateContext<UpdateParameters<'_>>,
    chart_id: i32,
    update_from_row: Option<DateValue>,
    min_blockscout_block: i64,
) -> Result<(), UpdateError>
where
    U: BatchUpdateableChart,
{
    let today = cx.user_context.current_time.date_naive();
    let txn = cx
        .user_context
        .blockscout
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

    let steps = generate_date_ranges(first_date, today, U::step_duration());
    let n = steps.len();

    for (i, range) in steps.into_iter().enumerate() {
        tracing::info!(from =? range.start(), to =? range.end() , "run {}/{} step of batch update", i + 1, n);
        let now = Instant::now();
        let found =
            batch_update_values_step::<U>(cx, chart_id, min_blockscout_block, range).await?;
        let elapsed: std::time::Duration = now.elapsed();
        tracing::info!(found =? found, elapsed =? elapsed, "{}/{} step of batch done", i + 1, n);
    }
    Ok(())
}

/// Returns how many records were found
async fn batch_update_values_step<U>(
    cx: &UpdateContext<UpdateParameters<'_>>,
    chart_id: i32,
    min_blockscout_block: i64,
    range: RangeInclusive<NaiveDate>,
) -> Result<usize, UpdateError>
where
    U: BatchUpdateableChart,
{
    let primary_data = U::PrimaryDependency::query_data(cx, range.clone()).await?;
    let secondary_data: <<U as BatchUpdateableChart>::SecondaryDependencies as DataSource>::Output =
        U::SecondaryDependencies::query_data(cx, range).await?;
    let found = U::batch_update_values_step_with(
        cx.user_context.db,
        chart_id,
        cx.user_context.current_time,
        min_blockscout_block,
        primary_data,
        secondary_data,
    )
    .await?;
    Ok(found)
}

impl<T: BatchUpdateableChart> UpdateableChart for BatchUpdateableChartWrapper<T> {
    type PrimaryDependency = T::PrimaryDependency;
    type SecondaryDependencies = T::SecondaryDependencies;

    async fn update_values(
        cx: &UpdateContext<UpdateParameters<'_>>,
        chart_id: i32,
        update_from_row: Option<DateValue>,
        min_blockscout_block: i64,
    ) -> Result<(), UpdateError> {
        batch_update_values::<T>(cx, chart_id, update_from_row, min_blockscout_block).await
    }
}

fn generate_date_ranges(
    start: NaiveDate,
    end: NaiveDate,
    step: Duration,
) -> Vec<RangeInclusive<NaiveDate>> {
    let mut date_range = Vec::new();
    let mut current_date = start;

    while current_date < end {
        // saturating add, since `step` is expected to be positive
        let next_date = current_date
            .checked_add_signed(step)
            .unwrap_or(NaiveDate::MAX);
        // todo: .min(end) ?
        date_range.push(RangeInclusive::new(current_date, next_date));
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
            let expected: Vec<_> = expected
                .into_iter()
                .map(|r| RangeInclusive::new(r.0, r.1))
                .collect();
            let actual = generate_date_ranges(from, to, Duration::days(30));
            assert_eq!(expected, actual);
        }
    }
}