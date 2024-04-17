use crate::{missing_date::get_and_fill_chart, DateValue, ExtendedDateValue, MissingDatePolicy};
use chrono::NaiveDate;
use entity::{chart_data, charts};
use sea_orm::{
    ColumnTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, QuerySelect, Statement,
};
use std::{cmp::min, collections::HashMap};
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
    date: NaiveDate,
    value: String,
}

pub async fn get_counters(
    db: &DatabaseConnection,
) -> Result<HashMap<String, DateValue>, ReadError> {
    let data = CounterData::find_by_statement(Statement::from_string(
        DbBackend::Postgres,
        r#"
            SELECT distinct on (charts.id) charts.name, data.date, data.value
            FROM "chart_data" "data"
            INNER JOIN "charts"
                ON data.chart_id = charts.id
            WHERE charts.chart_type = 'COUNTER'
            ORDER BY charts.id, data.id DESC;
        "#
        .into(),
    ))
    .all(db)
    .await?;

    let counters: HashMap<_, _> = data
        .into_iter()
        .map(|data| {
            (
                data.name,
                DateValue {
                    date: data.date,
                    value: data.value,
                },
            )
        })
        .collect();

    Ok(counters)
}

fn mark_approximate_data(data: Vec<DateValue>, mark_trailing_count: u64) -> Vec<ExtendedDateValue> {
    let mark_from = data
        .len()
        .saturating_sub(mark_trailing_count.try_into().unwrap_or(usize::MAX));
    data.into_iter()
        .enumerate()
        .map(|(i, dv)| ExtendedDateValue::from_date_value(dv, i >= mark_from))
        .collect()
}

pub async fn get_chart_data(
    db: &DatabaseConnection,
    name: &str,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    policy: Option<MissingDatePolicy>,
    approximate_trailing_values: u64,
) -> Result<Vec<ExtendedDateValue>, ReadError> {
    let chart = charts::Entity::find()
        .column(charts::Column::Id)
        .filter(charts::Column::Name.eq(name))
        .one(db)
        .await?
        .ok_or_else(|| ReadError::NotFound(name.into()))?;

    let to = match (to, chart.last_updated_at) {
        // No updates happened - no data should be present in DB
        // and no points should be returned.
        (None, None) => None,
        // Points up to `last_updated_at` should be retrieved (and filled).
        (None, Some(last_updated_at)) => Some(last_updated_at),
        // No updates happened - no data should be present in DB
        // and no points should be returned.
        (Some(to), None) => Some(to),
        // Respect `to` if requesting past data,
        // do not predict future otherwise.
        (Some(to), Some(last_updated_at)) => Some(min(to, last_updated_at)),
    };
    let (data, retrieved_points) = match policy {
        Some(policy) => get_and_fill_chart(db, chart.id, from, to, policy).await?,
        None => get_chart(db, chart.id, from, to).await?,
    };
    let data = mark_approximate_data(data, approximate_trailing_values);
    if chart.last_updated_at.is_none() && retrieved_points != 0 {
        tracing::warn!(
            chart_name = chart.name(),
            retrieved_db_points = retrieved_points,
            "`last_updated_at` is not set whereas data is present in DB"
        );
    }
    Ok(data)
}

async fn get_chart(
    db: &DatabaseConnection,
    chart_id: i32,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<Vec<DateValue>, DbErr> {
    let data_request = chart_data::Entity::find()
        .column(chart_data::Column::Date)
        .column(chart_data::Column::Value)
        .filter(chart_data::Column::ChartId.eq(chart_id))
        .order_by_asc(chart_data::Column::Date);

    let data_request = if let Some(from) = from {
        data_request.filter(chart_data::Column::Date.gte(from))
    } else {
        data_request
    };
    let data_request = if let Some(to) = to {
        data_request.filter(chart_data::Column::Date.lte(to))
    } else {
        data_request
    };
    let data = data_request.into_model().all(db).await?;
    (data, data.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{counters::TotalBlocks, tests::init_db::init_db, Chart};
    use entity::{chart_data, charts, sea_orm_active_enums::ChartType};
    use pretty_assertions::assert_eq;
    use sea_orm::{EntityTrait, Set};
    use std::str::FromStr;

    fn mock_chart_data(chart_id: i32, date: &str, value: i64) -> chart_data::ActiveModel {
        chart_data::ActiveModel {
            chart_id: Set(chart_id),
            date: Set(NaiveDate::from_str(date).unwrap()),
            value: Set(value.to_string()),
            ..Default::default()
        }
    }

    async fn insert_mock_data(db: &DatabaseConnection) {
        charts::Entity::insert_many([
            charts::ActiveModel {
                name: Set(TotalBlocks::default().name().to_string()),
                chart_type: Set(ChartType::Counter),
                ..Default::default()
            },
            charts::ActiveModel {
                name: Set("newBlocksPerDay".into()),
                chart_type: Set(ChartType::Line),
                ..Default::default()
            },
        ])
        .exec(db)
        .await
        .unwrap();
        chart_data::Entity::insert_many([
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

    fn value(date: &str, value: &str) -> DateValue {
        DateValue {
            date: NaiveDate::from_str(date).unwrap(),
            value: value.to_string(),
        }
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_counters_mock() {
        let _ = tracing_subscriber::fmt::try_init();

        let db = init_db("get_counters_mock").await;
        insert_mock_data(&db).await;
        let counters = get_counters(&db).await.unwrap();
        assert_eq!(
            HashMap::from_iter([("totalBlocks".into(), value("2022-11-12", "1350"))]),
            counters
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_chart_int_mock() {
        let _ = tracing_subscriber::fmt::try_init();

        let db = init_db("get_chart_int_mock").await;
        insert_mock_data(&db).await;
        let chart = get_chart_data(&db, "newBlocksPerDay", None, None, None, 1)
            .await
            .unwrap();
        assert_eq!(
            vec![
                ExtendedDateValue {
                    date: NaiveDate::from_str("2022-11-10").unwrap(),
                    value: "100".into(),
                    is_approximate: false,
                },
                ExtendedDateValue {
                    date: NaiveDate::from_str("2022-11-11").unwrap(),
                    value: "150".into(),
                    is_approximate: false,
                },
                ExtendedDateValue {
                    date: NaiveDate::from_str("2022-11-12").unwrap(),
                    value: "200".into(),
                    is_approximate: true,
                },
            ],
            chart
        );
    }
}
