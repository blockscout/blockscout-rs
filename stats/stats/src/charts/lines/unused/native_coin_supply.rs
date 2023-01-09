#![allow(unused_variables)]
use super::{lines_list, utils::OnlyDate};
use crate::{
    charts::insert::insert_int_data_many, tests::mock_lines::mocked_int_lines, UpdateError,
};
use async_trait::async_trait;
use entity::{
    chart_data_int,
    sea_orm_active_enums::{ChartType, ChartValueType},
};
use sea_orm::{prelude::*, QueryOrder, QuerySelect};

#[derive(Default, Debug)]
pub struct NativeCoinSupply {}

#[async_trait]
impl crate::Chart for NativeCoinSupply {
    fn name(&self) -> &str {
        lines_list::NATIVE_COIN_SUPPLY
    }

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        crate::charts::create_chart(db, self.name().into(), ChartType::Line, ChartValueType::Int)
            .await
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
    ) -> Result<(), UpdateError> {
        let id = crate::charts::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let last_row = chart_data_int::Entity::find()
            .column(chart_data_int::Column::Date)
            .filter(chart_data_int::Column::ChartId.eq(id))
            .order_by_desc(chart_data_int::Column::Date)
            .into_model::<OnlyDate>()
            .one(db)
            .await?;

        // TODO: remove mock
        let data = mocked_int_lines(1_000_000..100_000_000)
            .into_iter()
            .map(|item| item.active_model(id));
        insert_int_data_many(db, data).await?;
        Ok(())
    }
}
