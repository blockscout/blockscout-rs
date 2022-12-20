use async_trait::async_trait;
use blockscout_db::entity::blocks;
use chrono::NaiveDateTime;
use entity::{chart_data_int, charts};
use sea_orm::{
    sea_query, ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, QuerySelect, Set,
};

use super::UpdateError;

#[derive(FromQueryResult)]
struct TotalBlocksData {
    number: i64,
    timestamp: NaiveDateTime,
}

#[derive(Debug, FromQueryResult)]
struct ChartId {
    id: i32,
}

#[derive(Default, Debug)]
pub struct Updater {}

#[async_trait]
impl super::UpdaterTrait for Updater {
    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
    ) -> Result<(), UpdateError> {
        let name = "totalBlocksAllTime";
        let data = blocks::Entity::find()
            .column(blocks::Column::Number)
            .column(blocks::Column::Timestamp)
            .order_by_desc(blocks::Column::Timestamp)
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

        let id = charts::Entity::find()
            .column(charts::Column::Id)
            .filter(charts::Column::Name.eq(name))
            .into_model::<ChartId>()
            .one(db)
            .await?
            .ok_or_else(|| UpdateError::NotFound(name.into()))?;

        let data = chart_data_int::ActiveModel {
            id: Default::default(),
            chart_id: Set(id.id),
            date: Set(data.timestamp.date()),
            value: Set(data.number),
            created_at: Default::default(),
        };

        chart_data_int::Entity::insert(data)
            .on_conflict(
                sea_query::OnConflict::columns([
                    chart_data_int::Column::ChartId,
                    chart_data_int::Column::Date,
                ])
                .update_column(chart_data_int::Column::Value)
                .to_owned(),
            )
            .exec(db)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get_counters, UpdaterTrait};
    use blockscout_db::entity::{addresses, blocks};
    use chrono::{NaiveDate, NaiveDateTime};
    use entity::{
        charts,
        sea_orm_active_enums::{ChartType, ChartValueType},
    };
    use migration::MigratorTrait;
    use pretty_assertions::assert_eq;
    use sea_orm::{ConnectionTrait, Database, Statement};
    use std::str::FromStr;
    use url::Url;

    async fn init_db<M: MigratorTrait>(name: &str) -> DatabaseConnection {
        let db_url = std::env::var("DATABASE_URL").expect("no DATABASE_URL env");
        let url = Url::parse(&db_url).expect("unvalid database url");
        let db_url = url.join("/").unwrap().to_string();
        let raw_conn = Database::connect(db_url)
            .await
            .expect("failed to connect to postgres");

        raw_conn
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", name),
            ))
            .await
            .expect("failed to drop test database");
        raw_conn
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!("CREATE DATABASE {}", name),
            ))
            .await
            .expect("failed to create test database");

        let db_url = url.join(&format!("/{name}")).unwrap().to_string();
        let conn = Database::connect(db_url.clone())
            .await
            .expect("failed to connect to test db");
        M::up(&conn, None).await.expect("failed to run migrations");

        conn
    }

    async fn init_db_all(name: &str) -> (DatabaseConnection, DatabaseConnection) {
        let db = init_db::<migration::Migrator>(name).await;
        let blockscout =
            init_db::<blockscout_db::migration::Migrator>(&(name.to_owned() + "_blockscout")).await;
        (db, blockscout)
    }

    fn mock_block(index: i64, ts: &str) -> blocks::ActiveModel {
        blocks::ActiveModel {
            number: Set(index),
            hash: Set(index.to_le_bytes().to_vec()),
            timestamp: Set(NaiveDateTime::from_str(ts).unwrap()),
            consensus: Set(Default::default()),
            gas_limit: Set(Default::default()),
            gas_used: Set(Default::default()),
            miner_hash: Set(Default::default()),
            nonce: Set(Default::default()),
            parent_hash: Set(Default::default()),
            inserted_at: Set(Default::default()),
            updated_at: Set(Default::default()),
            ..Default::default()
        }
    }

    async fn mock_blockscout(blockscout: &DatabaseConnection, max_date: &str) {
        addresses::Entity::insert(addresses::ActiveModel {
            hash: Set(vec![]),
            inserted_at: Set(Default::default()),
            updated_at: Set(Default::default()),
            ..Default::default()
        })
        .exec(blockscout)
        .await
        .unwrap();

        let block_timestamps = vec![
            "2022-11-09T23:59:59",
            "2022-11-10T00:00:00",
            "2022-11-10T12:00:00",
            "2022-11-10T23:59:59",
            "2022-11-11T00:00:00",
            "2022-11-11T12:00:00",
            "2022-11-11T15:00:00",
            "2022-11-11T23:59:59",
            "2022-11-12T00:00:00",
        ]
        .into_iter()
        .filter(|val| {
            NaiveDateTime::from_str(val).unwrap().date() <= NaiveDate::from_str(max_date).unwrap()
        })
        .enumerate()
        .map(|(ind, ts)| mock_block(ind as i64, ts));
        blocks::Entity::insert_many(block_timestamps)
            .exec(blockscout)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_blocks_recurrent() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_total_blocks_recurrent").await;

        charts::Entity::insert(charts::ActiveModel {
            name: Set("totalBlocksAllTime".into()),
            chart_type: Set(ChartType::Counter),
            value_type: Set(ChartValueType::Int),
            ..Default::default()
        })
        .exec(&db)
        .await
        .unwrap();

        chart_data_int::Entity::insert(chart_data_int::ActiveModel {
            chart_id: Set(1),
            date: Set(NaiveDate::from_str("2022-11-10").unwrap()),
            value: Set(1),
            ..Default::default()
        })
        .exec(&db)
        .await
        .unwrap();

        mock_blockscout(&blockscout, "2022-11-11").await;

        Updater::default().update(&db, &blockscout).await.unwrap();
        let data = get_counters(&db).await.unwrap();
        assert_eq!("7", data.counters["totalBlocksAllTime"]);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_blocks_fresh() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_total_blocks_fresh").await;

        charts::Entity::insert(charts::ActiveModel {
            name: Set("totalBlocksAllTime".into()),
            chart_type: Set(ChartType::Counter),
            value_type: Set(ChartValueType::Int),
            ..Default::default()
        })
        .exec(&db)
        .await
        .unwrap();

        mock_blockscout(&blockscout, "2022-11-12").await;

        Updater::default().update(&db, &blockscout).await.unwrap();
        let data = get_counters(&db).await.unwrap();
        assert_eq!("8", data.counters["totalBlocksAllTime"]);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_blocks_last() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_total_blocks_last").await;

        charts::Entity::insert(charts::ActiveModel {
            name: Set("totalBlocksAllTime".into()),
            chart_type: Set(ChartType::Counter),
            value_type: Set(ChartValueType::Int),
            ..Default::default()
        })
        .exec(&db)
        .await
        .unwrap();

        chart_data_int::Entity::insert(chart_data_int::ActiveModel {
            chart_id: Set(1),
            date: Set(NaiveDate::from_str("2022-11-11").unwrap()),
            value: Set(1),
            ..Default::default()
        })
        .exec(&db)
        .await
        .unwrap();

        mock_blockscout(&blockscout, "2022-11-11").await;

        Updater::default().update(&db, &blockscout).await.unwrap();
        let data = get_counters(&db).await.unwrap();
        assert_eq!("7", data.counters["totalBlocksAllTime"]);
    }
}
