use crate::{
    charts::insert::{insert_data, DateValue},
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
        _blockscout: &DatabaseConnection,
        _full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = crate::charts::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;

        let item = DateValue {
            date: chrono::offset::Local::now().date_naive(),
            value: self.value.clone(),
        };
        insert_data(db, chart_id, item).await?;
        Ok(())
    }
}
