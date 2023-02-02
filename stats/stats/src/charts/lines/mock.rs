use crate::{
    charts::{insert::DateValue, updater::ChartFullUpdater},
    UpdateError,
};
use async_trait::async_trait;
use chrono::{Duration, NaiveDate};
use entity::sea_orm_active_enums::ChartType;
use rand::{distributions::uniform::SampleUniform, rngs::StdRng, Rng, SeedableRng};
use sea_orm::prelude::*;
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

pub fn mocked_lines<T: SampleUniform + PartialOrd + Clone + ToString>(
    range: Range<T>,
) -> Vec<DateValue> {
    let mut rng = StdRng::seed_from_u64(222);
    generate_intervals(NaiveDate::from_str("2022-01-01").unwrap())
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

#[async_trait]
impl<T: SampleUniform + PartialOrd + Clone + ToString + Send + Sync + 'static> ChartFullUpdater
    for MockLine<T>
{
    async fn get_values(
        &self,
        _blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValue>, UpdateError> {
        Ok(mocked_lines(self.range.clone()))
    }
}

#[async_trait]
impl<T: SampleUniform + PartialOrd + Clone + ToString + Send + Sync + 'static> crate::Chart
    for MockLine<T>
{
    fn name(&self) -> &str {
        &self.name
    }

    fn chart_type(&self) -> ChartType {
        ChartType::Line
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        self.update_with_values(db, blockscout, force_full).await
    }
}
