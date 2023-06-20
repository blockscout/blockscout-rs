use super::mutex::get_global_update_mutex;
use crate::{
    charts::updater::{get_last_row, get_min_block_blockscout},
    DateValue, ReadError,
};
use async_trait::async_trait;
use entity::{charts, sea_orm_active_enums::ChartType};
use sea_orm::{prelude::*, sea_query, FromQueryResult, QuerySelect, Set};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UpdateError {
    #[error("blockscout database error: {0}")]
    BlockscoutDB(DbErr),
    #[error("stats database error: {0}")]
    StatsDB(DbErr),
    #[error("chart {0} not found")]
    NotFound(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<ReadError> for UpdateError {
    fn from(read: ReadError) -> Self {
        match read {
            ReadError::DB(db) => UpdateError::StatsDB(db),
            ReadError::NotFound(err) => UpdateError::NotFound(err),
        }
    }
}

#[async_trait]
pub trait Chart: Sync {
    fn name(&self) -> &str;
    fn chart_type(&self) -> ChartType;

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        create_chart(db, self.name().into(), self.chart_type()).await
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError>;

    async fn update_with_mutex(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        let name = self.name();
        let mutex = get_global_update_mutex(name).await;
        let _permit = {
            match mutex.try_lock() {
                Ok(v) => v,
                Err(_) => {
                    tracing::warn!(
                        chart_name = name,
                        "found locked update mutex, waiting for unlock"
                    );
                    mutex.lock().await
                }
            }
        };
        self.update(db, blockscout, force_full).await
    }
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

pub struct UpdateInfo {
    pub chart_id: i32,
    pub min_blockscout_block: i64,
    pub last_row: Option<DateValue>,
}

pub async fn get_update_info<C>(
    chart: &C,
    db: &DatabaseConnection,
    blockscout: &DatabaseConnection,
    force_full: bool,
    last_row_offset: Option<u64>,
) -> Result<UpdateInfo, UpdateError>
where
    C: Chart + ?Sized,
{
    let chart_id = find_chart(db, chart.name())
        .await
        .map_err(UpdateError::StatsDB)?
        .ok_or_else(|| UpdateError::NotFound(chart.name().into()))?;
    let min_blockscout_block = get_min_block_blockscout(blockscout)
        .await
        .map_err(UpdateError::BlockscoutDB)?;
    let last_row = get_last_row(
        chart,
        chart_id,
        min_blockscout_block,
        db,
        force_full,
        last_row_offset,
    )
    .await?;

    Ok(UpdateInfo {
        chart_id,
        min_blockscout_block,
        last_row,
    })
}
