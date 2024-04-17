use crate::{
    missing_date::{fill_and_filter_chart, filter_within_range},
    DateValue, ExtendedDateValue, MissingDatePolicy,
};
use chrono::NaiveDate;
use entity::{chart_data, charts};
use sea_orm::{
    sea_query::Expr, ColumnTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Statement,
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

/// Mark corresponding data points as approximate.
///
/// Approximate are:
/// - points after `last_updated_at` date
/// - `mark_trailing_count` dates starting from `last_updated_at` and moving back
///
/// If `mark_trailing_count=0`` - only future points are marked
fn mark_approximate(
    data: Vec<DateValue>,
    last_updated_at: NaiveDate,
    mark_trailing_count: u64,
) -> Vec<ExtendedDateValue> {
    // saturating sub/add
    let next_after_updated_at = last_updated_at
        .checked_add_days(chrono::Days::new(1))
        .unwrap_or(NaiveDate::MAX);
    let mark_from_date = next_after_updated_at
        .checked_sub_days(chrono::Days::new(mark_trailing_count))
        .unwrap_or(NaiveDate::MIN);
    data.into_iter()
        .map(|dv| {
            let is_marked = dv.date >= mark_from_date;
            ExtendedDateValue::from_date_value(dv, is_marked)
        })
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

    let db_data = get_chart(db, chart.id, from, to).await?;

    let last_updated_at = chart.last_updated_at.map(|t| t.date_naive());
    if last_updated_at.is_none() && db_data.len() != 0 {
        tracing::warn!(
            chart_name = chart.name,
            db_data_len = db_data.len(),
            "`last_updated_at` is not set whereas data is present in DB"
        );
    }

    // may include future points that were not yet collected and were just filled accordingly.
    let data_with_maybe_future = match policy {
        Some(policy) => fill_and_filter_chart(db_data, from, to, policy),
        None => db_data,
    };
    let data_with_maybe_future_len = data_with_maybe_future.len();
    let data_unmarked = filter_within_range(data_with_maybe_future, None, last_updated_at);
    if let Some(filtered) = data_with_maybe_future_len.checked_sub(data_unmarked.len()) {
        if filtered > 0 {
            tracing::debug!(last_updated_at = ?last_updated_at, "{} points filled after last update were removed", filtered);
        }
    }
    let data = mark_approximate(
        data_unmarked,
        last_updated_at.unwrap_or(NaiveDate::MAX),
        approximate_trailing_values,
    );
    Ok(data)
}

async fn get_chart(
    db: &DatabaseConnection,
    chart_id: i32,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<Vec<DateValue>, DbErr> {
    let mut data_request = chart_data::Entity::find()
        .column(chart_data::Column::Date)
        .column(chart_data::Column::Value)
        .filter(chart_data::Column::ChartId.eq(chart_id))
        .order_by_asc(chart_data::Column::Date);

    if let Some(from) = from {
        let custom_where = Expr::cust_with_values::<sea_orm::sea_query::Value, _>(
            "date >= (SELECT COALESCE(MAX(date), '1900-01-01'::date) FROM chart_data WHERE chart_id = $1 AND date <= $2)",
            [chart_id.into(), from.into()],
        );
        QuerySelect::query(&mut data_request).cond_where(custom_where);
    }
    if let Some(to) = to {
        let custom_where = Expr::cust_with_values::<sea_orm::sea_query::Value, _>(
            "date <= (SELECT COALESCE(MIN(date), '9999-12-31'::date) FROM chart_data WHERE chart_id = $1 AND date >= $2)",
            [chart_id.into(), to.into()],
        );
        QuerySelect::query(&mut data_request).cond_where(custom_where);
    };

    let data = data_request.into_model().all(db).await?;
    Ok(data)
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
