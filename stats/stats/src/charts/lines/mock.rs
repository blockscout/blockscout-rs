use crate::{
    charts::db_interaction::{
        chart_updaters::{ChartFullUpdater, ChartUpdater},
        types::DateValue,
    },
    UpdateError,
};
use async_trait::async_trait;
use chrono::{Duration, NaiveDate};
use entity::sea_orm_active_enums::ChartType;
use rand::{distributions::uniform::SampleUniform, rngs::StdRng, Rng, SeedableRng};
use sea_orm::prelude::*;
use std::{ops::Range, str::FromStr};

fn generate_intervals(mut start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let mut times = vec![];
    while start < end {
        times.push(start);
        start += Duration::days(1);
    }
    times
}

pub fn mocked_lines<T: SampleUniform + PartialOrd + Clone + ToString>(
    range: Range<T>,
) -> Vec<DateValue> {
    let mut rng = StdRng::seed_from_u64(222);
    generate_intervals(
        NaiveDate::from_str("2022-01-01").unwrap(),
        NaiveDate::from_str("2022-04-01").unwrap(),
    )
    .into_iter()
    .map(|date| {
        let range = range.clone();
        let value = rng.gen_range(range);
        DateValue {
            date,
            value: value.to_string(),
        }
    })
    .collect()
}

#[derive(Debug)]
pub struct MockLine<T: SampleUniform + PartialOrd + Clone + ToString> {
    name: String,
    range: Range<T>,
}

impl<T: SampleUniform + PartialOrd + Clone + ToString> MockLine<T> {
    pub fn new(name: String, range: Range<T>) -> Self {
        Self { name, range }
    }
}

impl<T: SampleUniform + PartialOrd + Clone + ToString + Send + Sync + 'static> ChartFullUpdater
    for MockLine<T>
{
    async fn get_values(_blockscout: &DatabaseConnection) -> Result<Vec<DateValue>, UpdateError> {
        Ok(mocked_lines(self.range.clone()))
    }
}

#[async_trait]
impl<T: SampleUniform + PartialOrd + Clone + ToString + Send + Sync + 'static> crate::Chart
    for MockLine<T>
{
    fn name() -> &'static str {
        &self.name
    }

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

#[async_trait]
impl<T: SampleUniform + PartialOrd + Clone + ToString + Send + Sync + 'static> ChartUpdater
    for MockLine<T>
{
    async fn update_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        current_time: chrono::DateTime<chrono::Utc>,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        self.update_with_values(db, blockscout, current_time, force_full)
            .await
    }
}
