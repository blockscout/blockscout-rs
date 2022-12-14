use async_trait::async_trait;
use chrono::NaiveDate;
use entity::{chart_data_int, charts};
use sea_orm::{
    sea_query, ColumnTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait, FromQueryResult,
    QueryFilter, QuerySelect, Set, Statement,
};
use thiserror::Error;

#[async_trait]
pub trait Updater {
    async fn update(db: DatabaseConnection, blockscout: DatabaseConnection);
}

#[derive(Error, Debug)]
pub enum UpdateError {
    #[error("database error {0}")]
    DB(#[from] DbErr),
    #[error("chart {0} not found")]
    NotFound(String),
}

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
struct ChartId {
    id: i32,
}

pub async fn update_new_blocks_per_day(
    db: &DatabaseConnection,
    blockscout: &DatabaseConnection,
) -> Result<(), UpdateError> {
    let name = "new_blocks_per_day";
    let last_row = ChartData::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT distinct on (charts.id) charts.id, data.date 
            FROM "chart_data_int" "data"
            INNER JOIN "charts"
                ON data.chart_id = charts.id
            WHERE charts.name = $1
            ORDER BY charts.id, data.id DESC;
        "#
        .into(),
        vec![name.into()],
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
                "#
                .into(),
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
                .filter(charts::Column::Name.eq(name))
                .into_model::<ChartId>()
                .one(db)
                .await?
                .ok_or_else(|| UpdateError::NotFound(name.into()))?;
            (id.id, data)
        }
    };

    let data = data.into_iter().map(|row| chart_data_int::ActiveModel {
        id: Default::default(),
        chart_id: Set(id),
        date: Set(row.day),
        value: Set(row.count as i64),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_chart_int;
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

    async fn mock_blockscout(blockscout: &DatabaseConnection) {
        blockscout
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                r#"
            INSERT INTO public.addresses VALUES
                (0, 0, '', '', now(), now(), 0, false, false, 0, 0, 0);
                "#
                .into(),
            ))
            .await
            .unwrap();

        blockscout
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                r#"
            INSERT INTO public.blocks VALUES
                (false, 0, 0, 0, '0', '', '', 0, '', 0, '2022-11-09T23:59:59', 0, now(), now(), false, 0, false),
                (false, 0, 0, 0, '1', '', '', 1, '', 0, '2022-11-10T00:00:00', 0, now(), now(), false, 0, false),
                (false, 0, 0, 0, '2', '', '', 2, '', 0, '2022-11-10T12:00:00', 0, now(), now(), false, 0, false),
                (false, 0, 0, 0, '3', '', '', 3, '', 0, '2022-11-10T23:59:59', 0, now(), now(), false, 0, false),
                (false, 0, 0, 0, '4', '', '', 4, '', 0, '2022-11-11T00:00:00', 0, now(), now(), false, 0, false),
                (false, 0, 0, 0, '5', '', '', 5, '', 0, '2022-11-11T12:00:00', 0, now(), now(), false, 0, false),
                (false, 0, 0, 0, '6', '', '', 6, '', 0, '2022-11-11T15:00:00', 0, now(), now(), false, 0, false),
                (false, 0, 0, 0, '7', '', '', 7, '', 0, '2022-11-11T23:59:59', 0, now(), now(), false, 0, false),
                (false, 0, 0, 0, '8', '', '', 8, '', 0, '2022-11-12T00:00:00', 0, now(), now(), false, 0, false);
            "#
                .into(),
            ))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn update_new_blocks_recurrent() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_recurrent").await;

        charts::Entity::insert(charts::ActiveModel {
            name: Set("new_blocks_per_day".into()),
            r#type: Set(ChartType::Line),
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

        update_new_blocks_per_day(&db, &blockscout).await.unwrap();
        let data = get_chart_int(&db, "new_blocks_per_day").await.unwrap();
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
    async fn update_new_blocks_fresh() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_fresh").await;

        charts::Entity::insert(charts::ActiveModel {
            name: Set("new_blocks_per_day".into()),
            r#type: Set(ChartType::Line),
            value_type: Set(ChartValueType::Int),
            ..Default::default()
        })
        .exec(&db)
        .await
        .unwrap();

        mock_blockscout(&blockscout).await;

        update_new_blocks_per_day(&db, &blockscout).await.unwrap();
        let data = get_chart_int(&db, "new_blocks_per_day").await.unwrap();
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
    async fn update_new_blocks_last() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_no_change").await;

        charts::Entity::insert(charts::ActiveModel {
            name: Set("new_blocks_per_day".into()),
            r#type: Set(ChartType::Line),
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

        update_new_blocks_per_day(&db, &blockscout).await.unwrap();
        let data = get_chart_int(&db, "new_blocks_per_day").await.unwrap();
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
