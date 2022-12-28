#![allow(unused_variables)]
use super::{lines_list, utils::OnlyDate};
use crate::{
    charts::insert::insert_double_data_many, tests::mock_lines::mocked_double_lines, UpdateError,
};
use async_trait::async_trait;
use entity::{
    chart_data_int,
    sea_orm_active_enums::{ChartType, ChartValueType},
};
use sea_orm::{prelude::*, QueryOrder, QuerySelect};

#[derive(Default, Debug)]
pub struct AverageGasPrice {}

#[async_trait]
impl crate::Chart for AverageGasPrice {
    fn name(&self) -> &str {
        lines_list::AVERAGE_GAS_PRICE
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
        let data = mocked_double_lines(0.000000004..0.0001)
            .into_iter()
            .map(|item| item.active_model(id));
        insert_double_data_many(db, data).await?;
        Ok(())
    }
}
