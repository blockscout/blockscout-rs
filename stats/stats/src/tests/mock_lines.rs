use crate::charts::insert::{DoubleValueItem, IntValueItem};
use chrono::{Duration, NaiveDate};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::{ops::Range, str::FromStr};

fn generate_intervals(mut start: NaiveDate) -> Vec<NaiveDate> {
    let now = chrono::offset::Utc::now().naive_utc().date();
    let mut times = vec![];
    while start < now {
        times.push(start);
        start += Duration::days(1);
    }
    times
}

pub fn mocked_int_lines(range: Range<i64>) -> Vec<IntValueItem> {
    let mut rng = StdRng::seed_from_u64(222);
    generate_intervals(NaiveDate::from_str("2022-01-01").unwrap())
        .into_iter()
        .map(|date| {
            let range = range.clone();
            let value = rng.gen_range(range);
            IntValueItem { date, value }
        })
        .collect()
}

pub fn mocked_double_lines(range: Range<f64>) -> Vec<DoubleValueItem> {
    let mut rng = StdRng::seed_from_u64(222);
    generate_intervals(NaiveDate::from_str("2022-01-01").unwrap())
        .into_iter()
        .map(|date| {
            let range = range.clone();
            let value = rng.gen_range(range);
            DoubleValueItem { date, value }
        })
        .collect()
}
