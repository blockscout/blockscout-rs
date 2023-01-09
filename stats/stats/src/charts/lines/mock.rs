use crate::{
    charts::insert::{
        insert_double_data_many, insert_int_data_many, DoubleValueItem, IntValueItem,
    },
    UpdateError,
};
use async_trait::async_trait;
use chrono::{Duration, NaiveDate};
use entity::sea_orm_active_enums::{ChartType, ChartValueType};
use rand::{rngs::StdRng, Rng, SeedableRng};
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

#[derive(Debug)]
pub struct MockLineInt {
    name: String,
    range: Range<i64>,
}

impl MockLineInt {
    pub fn new(name: String, range: Range<i64>) -> Self {
        Self { name, range }
    }
}

#[async_trait]
impl crate::Chart for MockLineInt {
    fn name(&self) -> &str {
        &self.name
    }

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        crate::charts::create_chart(db, self.name().into(), ChartType::Line, ChartValueType::Int)
            .await
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        _blockscout: &DatabaseConnection,
    ) -> Result<(), UpdateError> {
        let id = crate::charts::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;

        let data = mocked_int_lines(self.range.clone())
            .into_iter()
            .map(|item| item.active_model(id));
        insert_int_data_many(db, data).await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct MockLineDouble {
    name: String,
    range: Range<f64>,
}

impl MockLineDouble {
    pub fn new(name: String, range: Range<f64>) -> Self {
        Self { name, range }
    }
}

#[async_trait]
impl crate::Chart for MockLineDouble {
    fn name(&self) -> &str {
        &self.name
    }

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        crate::charts::create_chart(
            db,
            self.name().into(),
            ChartType::Line,
            ChartValueType::Double,
        )
        .await
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        _blockscout: &DatabaseConnection,
    ) -> Result<(), UpdateError> {
        let id = crate::charts::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;

        let data = mocked_double_lines(self.range.clone())
            .into_iter()
            .map(|item| item.active_model(id));
        insert_double_data_many(db, data).await?;
        Ok(())
    }
}
