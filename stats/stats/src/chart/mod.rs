pub mod mock;
pub mod new_blocks;
pub mod total_blocks;

use async_trait::async_trait;
use entity::{
    charts,
    sea_orm_active_enums::{ChartType, ChartValueType},
};
use sea_orm::{sea_query, DatabaseConnection, DbErr, EntityTrait, Set};
use thiserror::Error;

#[async_trait]
pub trait Chart {
    fn name(&self) -> &str;

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr>;

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
    ) -> Result<(), UpdateError>;
}

#[derive(Error, Debug)]
pub enum UpdateError {
    #[error("database error {0}")]
    DB(#[from] DbErr),
    #[error("chart {0} not found")]
    NotFound(String),
}

pub async fn create_chart(
    db: &DatabaseConnection,
    name: String,
    chart_type: ChartType,
    value_type: ChartValueType,
) -> Result<(), DbErr> {
    charts::Entity::insert(charts::ActiveModel {
        name: Set(name),
        chart_type: Set(chart_type),
        value_type: Set(value_type),
        ..Default::default()
    })
    .on_conflict(
        sea_query::OnConflict::column(charts::Column::Name)
            .do_nothing()
            .to_owned(),
    )
    .exec(db)
    .await?;
    Ok(())
}
