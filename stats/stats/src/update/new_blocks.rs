use async_trait::async_trait;
use chrono::NaiveDate;
use entity::{chart_data_int, charts};
use sea_orm::{
    sea_query, ColumnTrait, DatabaseConnection, DbBackend, EntityTrait, FromQueryResult,
    QueryFilter, QuerySelect, Set, Statement,
};

use super::UpdateError;

#[derive(FromQueryResult)]
struct NewBlocksData {
    day: NaiveDate,
    count: i64,
}

#[derive(Debug, FromQueryResult)]
struct ChartData {
    id: i32,
    date: NaiveDate,
}

#[derive(Debug, FromQueryResult)]
struct ChartID {
    id: i32,
}

#[derive(Default, Debug)]
pub struct Updater {}

#[async_trait]
impl super::UpdaterTrait for Updater {
    fn name(&self) -> &str {
        "newBlocksPerDay"
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
    ) -> Result<(), UpdateError> {
        let last_row = ChartData::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT distinct on (charts.id) charts.id, data.date 
                FROM "chart_data_int" "data"
                INNER JOIN "charts"
                    ON data.chart_id = charts.id
                WHERE charts.name = $1
                ORDER BY charts.id, data.id DESC;
            "#,
            vec![self.name().into()],
        ))
        .one(db)
        .await?;
        let (id, data) = match last_row {
            Some(row) => {
                let data = NewBlocksData::find_by_statement(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    r#"
                    SELECT date(blocks.timestamp) as day, COUNT(*)
                        FROM public.blocks
                        WHERE date(blocks.timestamp) >= $1
                        GROUP BY day;
                    "#,
                    vec![row.date.into()],
                ))
                .all(blockscout)
                .await?;
                (row.id, data)
            }
            None => {
                let data = NewBlocksData::find_by_statement(Statement::from_string(
                    DbBackend::Postgres,
                    r#"
                    SELECT date(blocks.timestamp) as day, COUNT(*)
                        FROM public.blocks
                        GROUP BY day;
                    "#
                    .into(),
                ))
                .all(blockscout)
                .await?;
                let id = charts::Entity::find()
                    .column(charts::Column::Id)
                    .filter(charts::Column::Name.eq(self.name()))
                    .into_model::<ChartID>()
                    .one(db)
                    .await?
                    .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
                (id.id, data)
            }
        };

        let data = data.into_iter().map(|row| chart_data_int::ActiveModel {
            id: Default::default(),
            chart_id: Set(id),
            date: Set(row.day),
            value: Set(row.count),
            created_at: Default::default(),
        });

        chart_data_int::Entity::insert_many(data)
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
    use crate::{get_chart_int, UpdaterTrait};
    use blockscout_db::entity::{addresses, blocks};
    use chrono::NaiveDateTime;
    use entity::{
        charts,
        sea_orm_active_enums::{ChartType, ChartValueType},
    };
    use migration::MigratorTrait;
    use pretty_assertions::assert_eq;
    use sea_orm::{ConnectionTrait, Database, Statement};
    use stats_proto::blockscout::stats::v1::{LineChart, Point};
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

    async fn mock_blockscout(blockscout: &DatabaseConnection) {
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
        ];
        blocks::Entity::insert_many(
            block_timestamps
                .into_iter()
                .enumerate()
                .map(|(ind, ts)| mock_block(ind as i64, ts)),
        )
        .exec(blockscout)
        .await
        .unwrap();
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_blocks_recurrent() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_recurrent").await;

        let updater = Updater::default();
        charts::Entity::insert(charts::ActiveModel {
            name: Set(updater.name().into()),
            chart_type: Set(ChartType::Line),
            value_type: Set(ChartValueType::Int),
            ..Default::default()
        })
        .exec(&db)
        .await
        .unwrap();

        // set wrong value and check, that it was rewritten
        chart_data_int::Entity::insert(chart_data_int::ActiveModel {
            chart_id: Set(1),
            date: Set(NaiveDate::from_str("2022-11-10").unwrap()),
            value: Set(100),
            ..Default::default()
        })
        .exec(&db)
        .await
        .unwrap();

        mock_blockscout(&blockscout).await;

        updater.update(&db, &blockscout).await.unwrap();
        let data = get_chart_int(&db, updater.name(), None, None)
            .await
            .unwrap();
        let expected = LineChart {
            chart: vec![
                Point {
                    date: "2022-11-10".into(),
                    value: "3".into(),
                },
                Point {
                    date: "2022-11-11".into(),
                    value: "4".into(),
                },
                Point {
                    date: "2022-11-12".into(),
                    value: "1".into(),
                },
            ],
        };
        assert_eq!(expected, data);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_blocks_fresh() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_fresh").await;

        let updater = Updater::default();
        charts::Entity::insert(charts::ActiveModel {
            name: Set(updater.name().into()),
            chart_type: Set(ChartType::Line),
            value_type: Set(ChartValueType::Int),
            ..Default::default()
        })
        .exec(&db)
        .await
        .unwrap();

        mock_blockscout(&blockscout).await;

        updater.update(&db, &blockscout).await.unwrap();
        let data = get_chart_int(&db, updater.name(), None, None)
            .await
            .unwrap();
        let expected = LineChart {
            chart: vec![
                Point {
                    date: "2022-11-09".into(),
                    value: "1".into(),
                },
                Point {
                    date: "2022-11-10".into(),
                    value: "3".into(),
                },
                Point {
                    date: "2022-11-11".into(),
                    value: "4".into(),
                },
                Point {
                    date: "2022-11-12".into(),
                    value: "1".into(),
                },
            ],
        };
        assert_eq!(expected, data);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_blocks_last() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_last").await;

        let updater = Updater::default();
        charts::Entity::insert(charts::ActiveModel {
            name: Set(updater.name().into()),
            chart_type: Set(ChartType::Line),
            value_type: Set(ChartValueType::Int),
            ..Default::default()
        })
        .exec(&db)
        .await
        .unwrap();

        // set wrong values and check, that they wasn't rewritten
        // except the last one
        chart_data_int::Entity::insert_many([
            chart_data_int::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-09").unwrap()),
                value: Set(2),
                ..Default::default()
            },
            chart_data_int::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-10").unwrap()),
                value: Set(4),
                ..Default::default()
            },
            chart_data_int::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-11").unwrap()),
                value: Set(5),
                ..Default::default()
            },
            chart_data_int::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-12").unwrap()),
                value: Set(2),
                ..Default::default()
            },
        ])
        .exec(&db)
        .await
        .unwrap();

        mock_blockscout(&blockscout).await;

        updater.update(&db, &blockscout).await.unwrap();
        let data = get_chart_int(&db, updater.name(), None, None)
            .await
            .unwrap();
        let expected = LineChart {
            chart: vec![
                Point {
                    date: "2022-11-09".into(),
                    value: "2".into(),
                },
                Point {
                    date: "2022-11-10".into(),
                    value: "4".into(),
                },
                Point {
                    date: "2022-11-11".into(),
                    value: "5".into(),
                },
                Point {
                    date: "2022-11-12".into(),
                    value: "1".into(),
                },
            ],
        };
        assert_eq!(expected, data);
    }
}
