use crate::{
    charts::insert::{insert_int_data, IntValueItem},
    UpdateError,
};
use async_trait::async_trait;
use blockscout_db::entity::blocks;
use chrono::NaiveDateTime;
use entity::sea_orm_active_enums::{ChartType, ChartValueType};
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, FromQueryResult, QueryOrder, QuerySelect};

#[derive(FromQueryResult)]
struct TotalBlocksData {
    number: i64,
    timestamp: NaiveDateTime,
}

#[derive(Default, Debug)]
pub struct TotalBlocks {}

#[async_trait]
impl crate::Chart for TotalBlocks {
    fn name(&self) -> &str {
        "totalBlocks"
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
        let id = crate::charts::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;

        let data = blocks::Entity::find()
            .column(blocks::Column::Number)
            .column(blocks::Column::Timestamp)
            .order_by_desc(blocks::Column::Number)
            .into_model::<TotalBlocksData>()
            .one(blockscout)
            .await?;

        let data = match data {
            Some(data) => data,
            None => {
                tracing::warn!("no blocks data was found");
                return Ok(());
            }
        };
        let item = IntValueItem {
            date: data.timestamp.date(),
            value: data.number,
        };
        insert_int_data(db, id, item).await?;
        Ok(())
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
    use entity::chart_data_int;
    use pretty_assertions::assert_eq;
    use sea_orm::Set;
    use std::str::FromStr;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_blocks_recurrent() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_total_blocks_recurrent", None).await;
        let updater = TotalBlocks::default();

        updater.create(&db).await.unwrap();

        chart_data_int::Entity::insert(chart_data_int::ActiveModel {
            chart_id: Set(1),
            date: Set(NaiveDate::from_str("2022-11-10").unwrap()),
            value: Set(1),
            ..Default::default()
        })
        .exec(&db)
        .await
        .unwrap();

        fill_mock_blockscout_data(&blockscout, "2022-11-11").await;

        updater.update(&db, &blockscout).await.unwrap();
        let data = get_counters(&db).await.unwrap();
        assert_eq!("7", data.counters[updater.name()]);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_blocks_fresh() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_total_blocks_fresh", None).await;
        let updater = TotalBlocks::default();

        updater.create(&db).await.unwrap();

        fill_mock_blockscout_data(&blockscout, "2022-11-12").await;

        updater.update(&db, &blockscout).await.unwrap();
        let data = get_counters(&db).await.unwrap();
        assert_eq!("8", data.counters[updater.name()]);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_blocks_last() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_total_blocks_last", None).await;
        let updater = TotalBlocks::default();

        updater.create(&db).await.unwrap();

        chart_data_int::Entity::insert(chart_data_int::ActiveModel {
            chart_id: Set(1),
            date: Set(NaiveDate::from_str("2022-11-11").unwrap()),
            value: Set(1),
            ..Default::default()
        })
        .exec(&db)
        .await
        .unwrap();

        fill_mock_blockscout_data(&blockscout, "2022-11-11").await;

        updater.update(&db, &blockscout).await.unwrap();
        let data = get_counters(&db).await.unwrap();
        assert_eq!("7", data.counters[updater.name()]);
    }
}
