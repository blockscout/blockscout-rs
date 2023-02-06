use crate::{
    charts::{insert::DateValue, updater::ChartFullUpdater},
    UpdateError,
};
use async_trait::async_trait;
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
            date: chrono::offset::Local::now().date_naive(),
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

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        self.update_with_values(db, blockscout, force_full).await
    }
}
