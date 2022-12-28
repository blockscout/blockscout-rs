#![allow(unused_variables)]
use crate::{
    charts::insert::{insert_double_data, DoubleValueItem},
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::{ChartType, ChartValueType};
use sea_orm::prelude::*;

#[derive(Default, Debug)]
pub struct AverageBlockTime {}

#[async_trait]
impl crate::Chart for AverageBlockTime {
    fn name(&self) -> &str {
        super::counters_list::AVERAGE_BLOCK_TIME
    }

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
        blockscout: &DatabaseConnection,
    ) -> Result<(), UpdateError> {
        let chart_id = crate::charts::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        // TODO: remove mock
        let item = DoubleValueItem {
            date: chrono::offset::Local::now().date_naive(),
            value: 34.25,
        };
        insert_double_data(db, chart_id, item).await?;
        Ok(())
    }
}
