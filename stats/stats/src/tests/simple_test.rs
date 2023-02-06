use super::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data};
use crate::{get_chart_data, get_counters, Chart};
use pretty_assertions::assert_eq;
use sea_orm::DatabaseConnection;

pub async fn simple_test_chart(test_name: &str, chart: impl Chart, expected: Vec<(&str, &str)>) {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all(test_name, None).await;
    chart.create(&db).await.unwrap();
    fill_mock_blockscout_data(&blockscout, "2022-11-12").await;

    chart.update(&db, &blockscout, true).await.unwrap();
    _simple_test_chart(&db, &chart, &expected).await;

    chart.update(&db, &blockscout, false).await.unwrap();
    _simple_test_chart(&db, &chart, &expected).await;
}

async fn _simple_test_chart(
    db: &DatabaseConnection,
    chart: &impl Chart,
    expected: &Vec<(&str, &str)>,
) {
    let data = get_chart_data(db, chart.name(), None, None).await.unwrap();
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

pub async fn simple_test_counter(test_name: &str, counter: impl Chart, expected: &str) {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all(test_name, None).await;
    counter.create(&db).await.unwrap();
    fill_mock_blockscout_data(&blockscout, "2022-11-12").await;

    counter.update(&db, &blockscout, true).await.unwrap();
    _simple_test_counter(&db, &counter, expected).await;

    counter.update(&db, &blockscout, false).await.unwrap();
    _simple_test_counter(&db, &counter, expected).await;
}

async fn _simple_test_counter(db: &DatabaseConnection, counter: &impl Chart, expected: &str) {
    let data = get_counters(db).await.unwrap();
    let data = &data[counter.name()];
    assert_eq!(expected, data);
}
