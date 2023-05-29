use super::get_min_date_blockscout;
use crate::{DateValue, UpdateError};
use chrono::{Duration, NaiveDate, Utc};
use sea_orm::{DatabaseConnection, FromQueryResult, Statement, TransactionTrait};
use std::time::Instant;

pub async fn split_update<F>(
    blockscout: &DatabaseConnection,
    last_row: Option<DateValue>,
    query_maker: F,
) -> Result<Vec<DateValue>, UpdateError>
where
    F: Fn(NaiveDate, NaiveDate) -> Statement,
{
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

    let steps = generate_date_ranges(first_date, last_date);
    let n = steps.len();
    let mut results = vec![];

    for (i, (from_, to_)) in steps.into_iter().enumerate() {
        tracing::info!(from =? from_, to =? to_ , "run {}/{} step of split update", i + 1, n);
        let query = query_maker(from_, to_);
        let now = Instant::now();
        let data = DateValue::find_by_statement(query)
            .all(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        results.extend(data);
        let elapsed = now.elapsed();
        tracing::info!(elapsed =? elapsed, "{}/{} step of split done", i + 1, n);
    }
    Ok(results)
}

fn generate_date_ranges(start: NaiveDate, end: NaiveDate) -> Vec<(NaiveDate, NaiveDate)> {
    let mut date_range = Vec::new();
    let mut current_date = start;

    while current_date < end {
        let next_date = current_date + Duration::days(30);
        date_range.push((current_date, next_date));
        current_date = next_date + Duration::days(1);
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
                    (d("2022-02-01"), d("2022-03-03")),
                    (d("2022-03-04"), d("2022-04-03")),
                ],
            ),
            (
                (d("2015-07-20"), d("2015-12-31")),
                vec![
                    (d("2015-07-20"), d("2015-08-19")),
                    (d("2015-08-20"), d("2015-09-19")),
                    (d("2015-09-20"), d("2015-10-20")),
                    (d("2015-10-21"), d("2015-11-20")),
                    (d("2015-11-21"), d("2015-12-21")),
                    (d("2015-12-22"), d("2016-01-21")),
                ],
            ),
        ] {
            let actual = generate_date_ranges(from, to);
            assert_eq!(expected, actual);
        }
    }
}
