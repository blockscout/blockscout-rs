use super::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data};
use crate::{
    charts::db_interaction::updater::ChartUpdater, get_chart_data, get_counters, Chart,
    MissingDatePolicy,
};
use chrono::NaiveDate;
use sea_orm::DatabaseConnection;
use std::assert_eq;

pub async fn simple_test_chart(
    test_name: &str,
    chart: impl ChartUpdater,
    expected: Vec<(&str, &str)>,
) {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all(test_name).await;
    chart.create(&db).await.unwrap();
    fill_mock_blockscout_data(&blockscout, "2023-03-01").await;
    let approximate_until_updated = chart.approximate_until_updated();

    chart.update(&db, &blockscout, true).await.unwrap();
    get_chart_and_assert_eq(
        &db,
        &chart,
        &expected,
        None,
        None,
        None,
        approximate_until_updated,
    )
    .await;

    chart.update(&db, &blockscout, false).await.unwrap();
    get_chart_and_assert_eq(
        &db,
        &chart,
        &expected,
        None,
        None,
        None,
        approximate_until_updated,
    )
    .await;
}

pub async fn ranged_test_chart(
    test_name: &str,
    chart: impl ChartUpdater,
    expected: Vec<(&str, &str)>,
    from: NaiveDate,
    to: NaiveDate,
) {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all(test_name).await;
    chart.create(&db).await.unwrap();
    fill_mock_blockscout_data(&blockscout, "2023-03-01").await;
    let policy = chart.missing_date_policy();
    let approximate_until_updated = chart.approximate_until_updated();

    chart.update(&db, &blockscout, true).await.unwrap();
    get_chart_and_assert_eq(
        &db,
        &chart,
        &expected,
        Some(from),
        Some(to),
        Some(policy),
        approximate_until_updated,
    )
    .await;

    chart.update(&db, &blockscout, false).await.unwrap();
    get_chart_and_assert_eq(
        &db,
        &chart,
        &expected,
        Some(from),
        Some(to),
        Some(policy),
        approximate_until_updated,
    )
    .await;
}

async fn get_chart_and_assert_eq(
    db: &DatabaseConnection,
    chart: &impl Chart,
    expected: &Vec<(&str, &str)>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    policy: Option<MissingDatePolicy>,
    approximate_until_updated: u64,
) {
    let data = get_chart_data(
        db,
        chart.name(),
        from,
        to,
        policy,
        approximate_until_updated,
    )
    .await
    .unwrap();
    let data: Vec<_> = data
        .into_iter()
        .map(|p| (p.date.to_string(), p.value))
        .collect();
    let data: Vec<(&str, &str)> = data
        .iter()
        .map(|(date, value)| (date.as_str(), value.as_str()))
        .collect();
    assert_eq!(expected, &data);
}

pub async fn simple_test_counter(test_name: &str, counter: impl ChartUpdater, expected: &str) {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all(test_name).await;
    counter.create(&db).await.unwrap();
    fill_mock_blockscout_data(&blockscout, "2023-03-01").await;

    counter.update(&db, &blockscout, true).await.unwrap();
    get_counter_and_assert_eq(&db, &counter, expected).await;

    counter.update(&db, &blockscout, false).await.unwrap();
    get_counter_and_assert_eq(&db, &counter, expected).await;
}

async fn get_counter_and_assert_eq(db: &DatabaseConnection, counter: &impl Chart, expected: &str) {
    let data = get_counters(db).await.unwrap();
    let data = &data[counter.name()];
    let value = &data.value;
    assert_eq!(expected, value);
}
