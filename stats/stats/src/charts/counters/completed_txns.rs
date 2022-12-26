#![allow(unused_variables)]
use crate::{charts::insert::insert_int_data, UpdateError};
use async_trait::async_trait;
use entity::sea_orm_active_enums::{ChartType, ChartValueType};
use sea_orm::prelude::*;

#[derive(Default, Debug)]
pub struct CompletedTxns {}

#[async_trait]
impl crate::Chart for CompletedTxns {
    fn name(&self) -> &str {
        super::counters_list::COMPLETED_TXNS
    }

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
        blockscout: &DatabaseConnection,
    ) -> Result<(), UpdateError> {
        let chart_id = crate::charts::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;

        // TODO: remove mock
        insert_int_data(
            db,
            chart_id,
            chrono::offset::Local::now().date_naive(),
            956276037263,
        )
        .await?;
        Ok(())
    }
}
