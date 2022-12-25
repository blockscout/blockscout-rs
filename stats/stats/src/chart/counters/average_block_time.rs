#![allow(unused_variables)]
use crate::{chart::insert::insert_double_data, counters_list, UpdateError};
use async_trait::async_trait;
use entity::sea_orm_active_enums::{ChartType, ChartValueType};
use sea_orm::prelude::*;

#[derive(Default, Debug)]
pub struct AverageBlockTime {}

#[async_trait]
impl crate::Chart for AverageBlockTime {
    fn name(&self) -> &str {
        counters_list::AVERAGE_BLOCK_TIME
    }

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        crate::chart::create_chart(
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
        let chart_id = crate::chart::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        // TODO: remove mock
        insert_double_data(
            db,
            chart_id,
            chrono::offset::Local::now().date_naive(),
            34.25,
        )
        .await?;
        Ok(())
    }
}
