pub mod counters;
pub mod insert;
pub mod lines;

use async_trait::async_trait;
use chrono::NaiveDate;
use entity::{chart_data, charts, sea_orm_active_enums::ChartType};
use insert::{insert_data_many, DateValue};
use sea_orm::{prelude::*, sea_query, FromQueryResult, QueryOrder, QuerySelect, Set};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UpdateError {
    #[error("database error {0}")]
    DB(#[from] DbErr),
    #[error("chart {0} not found")]
    NotFound(String),
    #[error("internal error: {0}")]
    Internal(String),
}

#[async_trait]
pub trait Chart: Sync {
    fn name(&self) -> &str;
    fn chart_type(&self) -> ChartType;

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        crate::charts::create_chart(db, self.name().into(), self.chart_type()).await
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        full: bool,
    ) -> Result<(), UpdateError>;
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
) -> Result<(), DbErr> {
    let id = find_chart(db, &name).await?;
    if id.is_some() {
        return Ok(());
    }
    charts::Entity::insert(charts::ActiveModel {
        name: Set(name),
        chart_type: Set(chart_type),
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

#[async_trait]
pub trait ChartFullUpdater: Chart {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValue>, UpdateError>;

    async fn update_with_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        _full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = crate::charts::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let values = self
            .get_values(blockscout)
            .await?
            .into_iter()
            .map(|value| value.active_model(chart_id));
        insert_data_many(db, values).await?;
        Ok(())
    }
}

#[derive(Debug, FromQueryResult)]
pub struct OnlyDate {
    pub date: NaiveDate,
}

#[async_trait]
pub trait ChartUpdater: Chart {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
        last_row: Option<NaiveDate>,
    ) -> Result<Vec<DateValue>, UpdateError>;

    async fn update_with_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = crate::charts::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let last_row = if full {
            None
        } else {
            chart_data::Entity::find()
                .column(chart_data::Column::Date)
                .filter(chart_data::Column::ChartId.eq(chart_id))
                .order_by_desc(chart_data::Column::Date)
                .into_model::<OnlyDate>()
                .one(db)
                .await?
        };
        let values = self
            .get_values(blockscout, last_row.map(|row| row.date))
            .await?
            .into_iter()
            .map(|value| value.active_model(chart_id));
        insert_data_many(db, values).await?;
        Ok(())
    }
}
