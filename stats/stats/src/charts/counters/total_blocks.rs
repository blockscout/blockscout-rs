use crate::{
    charts::{insert::DateValue, updater::ChartFullUpdater},
    UpdateError,
};
use async_trait::async_trait;
use blockscout_db::entity::blocks;
use chrono::NaiveDateTime;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, sea_query::Expr, FromQueryResult, QuerySelect};

#[derive(FromQueryResult)]
struct TotalBlocksData {
    number: i64,
    timestamp: NaiveDateTime,
}

#[derive(Default, Debug)]
pub struct TotalBlocks {}

#[async_trait]
impl ChartFullUpdater for TotalBlocks {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let data = blocks::Entity::find()
            .select_only()
            .column_as(Expr::col(blocks::Column::Number).count(), "number")
            .column_as(Expr::col(blocks::Column::Timestamp).max(), "timestamp")
            .filter(blocks::Column::Consensus.eq(true))
            .into_model::<TotalBlocksData>()
            .one(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?
            .ok_or_else(|| UpdateError::Internal("query returned nothing".into()))?;

        let data = DateValue {
            date: data.timestamp.date(),
            value: data.number.to_string(),
        };
        Ok(vec![data])
    }
}

#[async_trait]
impl crate::Chart for TotalBlocks {
    fn name(&self) -> &str {
        "totalBlocks"
    }

    fn chart_type(&self) -> ChartType {
        ChartType::Counter
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        self.update_with_values(db, blockscout, force_full).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        get_counters,
        tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
        Chart,
    };
    use chrono::NaiveDate;
    use entity::chart_data;
    use pretty_assertions::assert_eq;
    use sea_orm::Set;
    use std::str::FromStr;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_blocks_recurrent() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_total_blocks_recurrent").await;
        let updater = TotalBlocks::default();

        updater.create(&db).await.unwrap();

        chart_data::Entity::insert(chart_data::ActiveModel {
            chart_id: Set(1),
            date: Set(NaiveDate::from_str("2022-11-10").unwrap()),
            value: Set(1.to_string()),
            ..Default::default()
        })
        .exec(&db as &DatabaseConnection)
        .await
        .unwrap();

        fill_mock_blockscout_data(&blockscout, "2023-03-01").await;

        updater.update(&db, &blockscout, true).await.unwrap();
        let data = get_counters(&db).await.unwrap();
        assert_eq!("13", data[updater.name()].value);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_blocks_fresh() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_total_blocks_fresh").await;
        let updater = TotalBlocks::default();

        updater.create(&db).await.unwrap();

        fill_mock_blockscout_data(&blockscout, "2022-11-12").await;

        updater.update(&db, &blockscout, true).await.unwrap();
        let data = get_counters(&db).await.unwrap();
        assert_eq!("9", data[updater.name()].value);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_blocks_last() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_total_blocks_last").await;
        let updater = TotalBlocks::default();

        updater.create(&db).await.unwrap();

        chart_data::Entity::insert(chart_data::ActiveModel {
            chart_id: Set(1),
            date: Set(NaiveDate::from_str("2022-11-11").unwrap()),
            value: Set(1.to_string()),
            ..Default::default()
        })
        .exec(&db as &DatabaseConnection)
        .await
        .unwrap();

        fill_mock_blockscout_data(&blockscout, "2023-03-01").await;

        updater.update(&db, &blockscout, true).await.unwrap();
        let data = get_counters(&db).await.unwrap();
        assert_eq!("13", data[updater.name()].value);
    }
}
