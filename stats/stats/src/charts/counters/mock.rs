use crate::{
    charts::insert::{insert_double_data, insert_int_data, DoubleValueItem, IntValueItem},
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::{ChartType, ChartValueType};
use sea_orm::prelude::*;

#[derive(Debug)]
pub struct MockCounterInt {
    name: String,
    value: i64,
}

impl MockCounterInt {
    pub fn new(name: String, value: i64) -> Self {
        Self { name, value }
    }
}

#[async_trait]
impl crate::Chart for MockCounterInt {
    fn name(&self) -> &str {
        &self.name
    }

    fn chart_type(&self) -> ChartType {
        ChartType::Counter
    }

    // TODO: remove when we remove chart value type
    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        crate::charts::create_chart(
            db,
            self.name().into(),
            ChartType::Counter,
            ChartValueType::Int,
        )
        .await
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

        let item = IntValueItem {
            date: chrono::offset::Local::now().date_naive(),
            value: self.value,
        };
        insert_int_data(db, chart_id, item).await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct MockCounterDouble {
    name: String,
    value: f64,
}

impl MockCounterDouble {
    pub fn new(name: String, value: f64) -> Self {
        Self { name, value }
    }
}

#[async_trait]
impl crate::Chart for MockCounterDouble {
    fn name(&self) -> &str {
        &self.name
    }

    fn chart_type(&self) -> ChartType {
        ChartType::Counter
    }

    // TODO: remove when we remove chart value type
    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        crate::charts::create_chart(
            db,
            self.name().into(),
            ChartType::Counter,
            ChartValueType::Double,
        )
        .await
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

        let item = DoubleValueItem {
            date: chrono::offset::Local::now().date_naive(),
            value: self.value,
        };
        insert_double_data(db, chart_id, item).await?;
        Ok(())
    }
}
