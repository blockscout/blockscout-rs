use crate::{
    charts::db_interaction::{
        chart_updaters::{ChartFullUpdater, ChartUpdater},
        types::DateValue,
    },
    UpdateError,
};
use async_trait::async_trait;
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;

#[derive(Debug)]
pub struct MockCounter {
    name: String,
    value: String,
}

impl MockCounter {
    pub fn new(name: String, value: String) -> Self {
        Self { name, value }
    }
}

#[async_trait]
impl ChartFullUpdater for MockCounter {
    async fn get_values(
        &self,
        _blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let item = DateValue {
            date: NaiveDate::parse_from_str("2022-11-12", "%Y-%m-%d").unwrap(),
            value: self.value.clone(),
        };
        Ok(vec![item])
    }
}

#[async_trait]
impl crate::Chart for MockCounter {
    fn name(&self) -> &str {
        &self.name
    }

    fn chart_type(&self) -> ChartType {
        ChartType::Counter
    }
}

#[async_trait]
impl ChartUpdater for MockCounter {
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
