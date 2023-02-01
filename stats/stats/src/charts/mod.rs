pub mod counters;
pub mod insert;
pub mod lines;

use crate::metrics;
use async_trait::async_trait;
use blockscout_db::entity::blocks;
use chrono::NaiveDate;
use entity::{chart_data, charts, sea_orm_active_enums::ChartType};
use insert::{insert_data_many, DateValue};
use sea_orm::{prelude::*, sea_query, FromQueryResult, QueryOrder, QuerySelect, Set};
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
        force_full: bool,
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
        _force_full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = crate::charts::find_chart(db, self.name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let values = {
            let _timer = metrics::CHART_FETCH_NEW_DATA_TIME
                .with_label_values(&[self.name()])
                .start_timer();
            self.get_values(blockscout)
                .await?
                .into_iter()
                .map(|value| value.active_model(chart_id, None))
        };
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        Ok(())
    }
}

#[derive(Debug, FromQueryResult)]
struct SyncInfo {
    pub date: NaiveDate,
    pub min_blockscout_block: Option<i64>,
}

#[derive(FromQueryResult)]
struct MinBlock {
    min_block: i64,
}

async fn get_min_block_blockscout(blockscout: &DatabaseConnection) -> Result<i64, DbErr> {
    let min_block = blocks::Entity::find()
        .select_only()
        .column_as(
            sea_query::Expr::col(blocks::Column::Number).min(),
            "min_block",
        )
        .filter(blocks::Column::Consensus.eq(true))
        .into_model::<MinBlock>()
        .one(blockscout)
        .await?;

    min_block
        .map(|r| r.min_block)
        .ok_or_else(|| DbErr::RecordNotFound("no blocks found in blockscout database".into()))
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
        force_full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = crate::charts::find_chart(db, self.name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let min_blockscout_block = get_min_block_blockscout(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let last_row: Option<NaiveDate> = if force_full {
            tracing::info!(
                min_blockscout_block = min_blockscout_block,
                chart = self.name(),
                "running full update due to force override"
            );
            None
        } else {
            let last_row = chart_data::Entity::find()
                .column(chart_data::Column::Date)
                .filter(chart_data::Column::ChartId.eq(chart_id))
                .order_by_desc(chart_data::Column::Date)
                .into_model::<SyncInfo>()
                .one(db)
                .await
                .map_err(UpdateError::StatsDB)?;

            match last_row {
                Some(row) => {
                    if let Some(block) = row.min_blockscout_block {
                        if block != min_blockscout_block {
                            tracing::info!(
                                min_blockscout_block = min_blockscout_block,
                                min_chart_block = block,
                                chart = self.name(),
                                "running partial update"
                            );
                            Some(row.date)
                        } else {
                            tracing::info!(
                                min_blockscout_block = min_blockscout_block,
                                min_chart_block = block,
                                chart = self.name(),
                                "running full update due to min blocks mismatch"
                            );
                            None
                        }
                    } else {
                        tracing::info!(
                            min_blockscout_block = min_blockscout_block,
                            chart = self.name(),
                            "running full update due to lack of saved min block"
                        );
                        None
                    }
                }
                None => {
                    tracing::info!(
                        min_blockscout_block = min_blockscout_block,
                        chart = self.name(),
                        "running full update due to lack of history data"
                    );
                    None
                }
            }
        };
        let values = {
            let _timer = metrics::CHART_FETCH_NEW_DATA_TIME
                .with_label_values(&[self.name()])
                .start_timer();
            self.get_values(blockscout, last_row)
                .await?
                .into_iter()
                .map(|value| value.active_model(chart_id, Some(min_blockscout_block)))
        };
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(())
    }
}
