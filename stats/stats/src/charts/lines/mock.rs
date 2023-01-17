use crate::{
    charts::insert::{insert_data_many, DateValue},
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
        _blockscout: &DatabaseConnection,
        _full: bool,
    ) -> Result<(), UpdateError> {
        let id = crate::charts::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;

        let data = mocked_lines(self.range.clone())
            .into_iter()
            .map(|item| item.active_model(id));
        insert_data_many(db, data).await?;
        Ok(())
    }
}
