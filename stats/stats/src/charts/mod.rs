pub mod counters;
pub mod insert;
pub mod lines;

use async_trait::async_trait;
use entity::{
    charts,
    sea_orm_active_enums::{ChartType, ChartValueType},
};
use sea_orm::{prelude::*, sea_query, FromQueryResult, QuerySelect, Set};
use thiserror::Error;

#[async_trait]
pub trait Chart {
    fn name(&self) -> &str;

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr>;

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        full: bool,
    ) -> Result<(), UpdateError>;
}

#[derive(Error, Debug)]
pub enum UpdateError {
    #[error("database error {0}")]
    DB(#[from] DbErr),
    #[error("chart {0} not found")]
    NotFound(String),
}

#[derive(Debug, FromQueryResult)]
struct ChartID {
    id: i32,
}

pub async fn find_chart(db: &DatabaseConnection, name: &str) -> Result<Option<i32>, DbErr> {
    charts::Entity::find()
        .column(charts::Column::Id)
        .filter(charts::Column::Name.eq(name))
        .into_model::<ChartID>()
        .one(db)
        .await
        .map(|id| id.map(|id| id.id))
}

pub async fn create_chart(
    db: &DatabaseConnection,
    name: String,
    chart_type: ChartType,
    value_type: ChartValueType,
) -> Result<(), DbErr> {
    let id = find_chart(db, &name).await?;
    if id.is_some() {
        return Ok(());
    }
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
