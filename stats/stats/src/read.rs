use crate::charts::insert::{DoubleValueItem, IntValueItem};
use chrono::NaiveDate;
use entity::{chart_data_double, chart_data_int, charts, sea_orm_active_enums::ChartValueType};
use sea_orm::{
    ColumnTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, QuerySelect, Statement,
};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReadError {
    #[error("database error {0}")]
    DB(#[from] DbErr),
    #[error("chart {0} not found")]
    NotFound(String),
}

#[derive(Debug, FromQueryResult)]
struct CounterData {
    name: String,
    value: String,
}

#[derive(Debug, Clone, Copy)]
enum Data {
    Int,
    Double,
}

pub async fn get_counters(db: &DatabaseConnection) -> Result<HashMap<String, String>, ReadError> {
    let int_counters = get_counters_data(db, Data::Int).await?;
    let double_counters = get_counters_data(db, Data::Double).await?;

    let counters = int_counters.into_iter().chain(double_counters).collect();
    Ok(counters)
}

async fn get_counters_data(
    db: &DatabaseConnection,
    data: Data,
) -> Result<HashMap<String, String>, ReadError> {
    let table_name = match data {
        Data::Int => "chart_data_int",
        Data::Double => "chart_data_double",
    };
    let data = CounterData::find_by_statement(Statement::from_string(
        DbBackend::Postgres,
        format!(
            r#"
            SELECT distinct on (charts.id) charts.name, data.value::text
            FROM "{}" "data"
            INNER JOIN "charts"
                ON data.chart_id = charts.id
            WHERE charts.chart_type = 'COUNTER'
            ORDER BY charts.id, data.id DESC;
        "#,
            table_name
        ),
    ))
    .all(db)
    .await?;

    let counters: HashMap<_, _> = data
        .into_iter()
        .map(|data| (data.name, data.value))
        .collect();

    Ok(counters)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Point {
    pub date: NaiveDate,
    pub value: String,
}

pub async fn get_chart_data(
    db: &DatabaseConnection,
    name: &str,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<Vec<Point>, ReadError> {
    let chart = charts::Entity::find()
        .column(charts::Column::Id)
        .filter(charts::Column::Name.eq(name))
        .one(db)
        .await?
        .ok_or_else(|| ReadError::NotFound(name.into()))?;

    let chart = match chart.value_type {
        ChartValueType::Int => get_chart_int(db, chart.id, from, to)
            .await?
            .into_iter()
            .map(|row| Point {
                date: row.date,
                value: row.value.to_string(),
            })
            .collect(),
        ChartValueType::Double => get_chart_double(db, chart.id, from, to)
            .await?
            .into_iter()
            .map(|row| Point {
                date: row.date,
                value: row.value.to_string(),
            })
            .collect(),
    };
    Ok(chart)
}

async fn get_chart_int(
    db: &DatabaseConnection,
    chart_id: i32,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<Vec<IntValueItem>, DbErr> {
    let data_request = chart_data_int::Entity::find()
        .column(chart_data_int::Column::Date)
        .column(chart_data_int::Column::Value)
        .filter(chart_data_int::Column::ChartId.eq(chart_id))
        .order_by_asc(chart_data_int::Column::Date);

    let data_request = if let Some(from) = from {
        data_request.filter(chart_data_int::Column::Date.gte(from))
    } else {
        data_request
    };
    let data_request = if let Some(to) = to {
        data_request.filter(chart_data_int::Column::Date.lte(to))
    } else {
        data_request
    };
    data_request.into_model().all(db).await
}

async fn get_chart_double(
    db: &DatabaseConnection,
    chart_id: i32,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<Vec<DoubleValueItem>, DbErr> {
    let data_request = chart_data_double::Entity::find()
        .column(chart_data_double::Column::Date)
        .column(chart_data_double::Column::Value)
        .filter(chart_data_double::Column::ChartId.eq(chart_id))
        .order_by_asc(chart_data_double::Column::Date);

    let data_request = if let Some(from) = from {
        data_request.filter(chart_data_double::Column::Date.gte(from))
    } else {
        data_request
    };
    let data_request = if let Some(to) = to {
        data_request.filter(chart_data_double::Column::Date.lte(to))
    } else {
        data_request
    };
    data_request.into_model().all(db).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{counters::TotalBlocks, tests::init_db::init_db, Chart};
    use entity::{
        chart_data_int, charts,
        sea_orm_active_enums::{ChartType, ChartValueType},
    };
    use pretty_assertions::assert_eq;
    use sea_orm::{EntityTrait, Set};
    use std::str::FromStr;

    fn mock_chart_data(chart_id: i32, date: &str, value: i64) -> chart_data_int::ActiveModel {
        chart_data_int::ActiveModel {
            chart_id: Set(chart_id),
            date: Set(NaiveDate::from_str(date).unwrap()),
            value: Set(value),
            ..Default::default()
        }
    }

    async fn insert_mock_data(db: &DatabaseConnection) {
        charts::Entity::insert_many([
            charts::ActiveModel {
                name: Set(TotalBlocks::default().name().to_string()),
                chart_type: Set(ChartType::Counter),
                value_type: Set(ChartValueType::Int),
                ..Default::default()
            },
            charts::ActiveModel {
                name: Set("newBlocksPerDay".into()),
                chart_type: Set(ChartType::Line),
                value_type: Set(ChartValueType::Int),
                ..Default::default()
            },
        ])
        .exec(db)
        .await
        .unwrap();
        chart_data_int::Entity::insert_many([
            mock_chart_data(1, "2022-11-10", 1000),
            mock_chart_data(2, "2022-11-10", 100),
            mock_chart_data(1, "2022-11-11", 1150),
            mock_chart_data(2, "2022-11-11", 150),
            mock_chart_data(1, "2022-11-12", 1350),
            mock_chart_data(2, "2022-11-12", 200),
        ])
        .exec(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_counters_mock() {
        let _ = tracing_subscriber::fmt::try_init();

        let db = init_db::<migration::Migrator>("get_counters_mock", None).await;
        insert_mock_data(&db).await;
        let counters = get_counters(&db).await.unwrap();
        assert_eq!(
            HashMap::from_iter([("totalBlocks".into(), "1350".into())]),
            counters
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_chart_int_mock() {
        let _ = tracing_subscriber::fmt::try_init();

        let db = init_db::<migration::Migrator>("get_chart_int_mock", None).await;
        insert_mock_data(&db).await;
        let chart = get_chart_data(&db, "newBlocksPerDay", None, None)
            .await
            .unwrap();
        assert_eq!(
            vec![
                Point {
                    date: NaiveDate::from_str("2022-11-10").unwrap(),
                    value: "100".into(),
                },
                Point {
                    date: NaiveDate::from_str("2022-11-11").unwrap(),
                    value: "150".into(),
                },
                Point {
                    date: NaiveDate::from_str("2022-11-12").unwrap(),
                    value: "200".into(),
                },
            ],
            chart
        );
    }
}
