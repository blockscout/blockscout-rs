use chrono::NaiveDateTime;
use sea_orm::{DatabaseConnection, DbBackend, DbErr, FromQueryResult, Statement};
use stats_proto::blockscout::stats::v1::{Counters, LineChart, Point};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReadError {
    #[error("database error {0}")]
    DB(#[from] DbErr),
    #[error("chart {0} not found")]
    NotFound(String),
}

#[derive(FromQueryResult)]
struct CounterData {
    name: String,
    date: NaiveDateTime,
    value: i64,
}

fn get_counter_from_data(
    data: &HashMap<String, (NaiveDateTime, u64)>,
    name: &str,
) -> Result<u64, ReadError> {
    data.get(name)
        .map(|(_, value)| *value)
        .ok_or_else(|| ReadError::NotFound(name.into()))
}

pub async fn get_counters(db: &DatabaseConnection) -> Result<Counters, ReadError> {
    let data = CounterData::find_by_statement(Statement::from_string(
        DbBackend::Postgres,
        r#"
        SELECT distinct on (charts.id) charts.name, data.date, data.value 
            FROM "chart_data_int" "data"
            INNER JOIN "charts"
                ON data.chart_id = charts.id
            WHERE charts.type = 'COUNTER'
            ORDER BY charts.id, data.id DESC;
        "#
        .into(),
    ))
    .all(db)
    .await?;
    let data: HashMap<_, _> = data
        .into_iter()
        .map(|data| (data.name, (data.date, data.value as u64)))
        .collect();
    let counters = Counters {
        counters: HashMap::from_iter([(
            "totalBlocksAllTime".into(),
            get_counter_from_data(&data, "totalBlocksAllTime")?.to_string(),
        )]),
    };
    Ok(counters)
}

#[derive(FromQueryResult)]
struct DateValue {
    date: NaiveDateTime,
    value: i64,
}

pub async fn get_chart_int(db: &DatabaseConnection, name: &str) -> Result<LineChart, DbErr> {
    let data = DateValue::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
            SELECT data.date, data.value 
                FROM "chart_data_int" "data"
                INNER JOIN "charts"
                    ON data.chart_id = charts.id
                WHERE charts.name = $1
                ORDER BY data.date;
            "#,
        vec![name.into()],
    ))
    .all(db)
    .await?;
    let chart = data
        .into_iter()
        .map(|row| Point {
            date: row.date.format("%Y-%m-%d").to_string(),
            value: row.value.to_string(),
        })
        .collect();
    Ok(LineChart { chart })
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::MigratorTrait;
    use pretty_assertions::assert_eq;
    use sea_orm::{ConnectionTrait, Database};
    use url::Url;

    async fn init_db(name: &str) -> DatabaseConnection {
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
        migration::Migrator::up(&conn, None)
            .await
            .expect("failed to run migrations");

        conn
    }

    async fn insert_mock_data(db: &DatabaseConnection) {
        db.query_one(Statement::from_string(
            DbBackend::Postgres,
            r#"
            INSERT INTO "charts" VALUES
                (default, 'totalBlocksAllTime', 'COUNTER', 'INT', 0),
                (default, 'newBlocksPerDay', 'LINE', 'INT', 0),
                (default, 'totalTxsAllTime', 'COUNTER', 'INT', 0),
                (default, 'newTxsPerDay', 'LINE', 'INT', 0);
            "#
            .into(),
        ))
        .await
        .unwrap();
        db.query_one(Statement::from_string(
            DbBackend::Postgres,
            r#"
            INSERT INTO "chart_data_int" VALUES
                (default, 1, date '2022-11-10', 1000),
                (default, 2, date '2022-11-10', 100),
                (default, 3, date '2022-11-10', 3000),
                (default, 4, date '2022-11-10', 300),

                (default, 1, date '2022-11-11', 1150),
                (default, 2, date '2022-11-11', 150),
                (default, 3, date '2022-11-11', 3350),
                (default, 4, date '2022-11-11', 350),

                (default, 1, date '2022-11-12', 1350),
                (default, 2, date '2022-11-12', 200),
                (default, 3, date '2022-11-12', 3750),
                (default, 4, date '2022-11-12', 400);
            "#
            .into(),
        ))
        .await
        .unwrap();
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_counters_mock() {
        let _ = tracing_subscriber::fmt::try_init();

        let db = init_db("get_counters_mock").await;
        insert_mock_data(&db).await;
        let counters = get_counters(&db).await.unwrap();
        assert_eq!(
            Counters {
                counters: HashMap::from_iter([("totalBlocksAllTime".into(), "1350".into())]),
            },
            counters
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_chart_int_mock() {
        let _ = tracing_subscriber::fmt::try_init();

        let db = init_db("get_chart_int_mock").await;
        insert_mock_data(&db).await;
        let chart = get_chart_int(&db, "newBlocksPerDay").await.unwrap();
        assert_eq!(
            LineChart {
                chart: vec![
                    Point {
                        date: "2022-11-10".into(),
                        value: "100".into(),
                    },
                    Point {
                        date: "2022-11-11".into(),
                        value: "150".into(),
                    },
                    Point {
                        date: "2022-11-12".into(),
                        value: "200".into(),
                    },
                ]
            },
            chart
        );
    }
}
