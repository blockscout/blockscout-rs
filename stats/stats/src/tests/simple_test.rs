use super::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data};
use crate::{
    charts::{
        data_source::{UpdateContext, UpdateParameters},
        db_interaction::chart_updaters::ChartUpdater,
    },
    get_chart_data, get_counters, Chart, MissingDatePolicy,
};
use chrono::{DateTime, NaiveDate};
use sea_orm::DatabaseConnection;
use std::{assert_eq, str::FromStr};

pub async fn simple_test_chart<C: ChartUpdater>(test_name: &str, expected: Vec<(&str, &str)>) {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all(test_name).await;
    let current_time = DateTime::from_str("2023-03-01T12:00:00Z").unwrap();
    let current_date = current_time.date_naive();
    // C::create(&db).await.unwrap();
    fill_mock_blockscout_data(&blockscout, current_date).await;
    let approximate_trailing_points = C::approximate_trailing_points();

    let mut parameters = UpdateParameters {
        db: &db,
        blockscout: &blockscout,
        current_time,
        force_full: true,
    };
    let mut cx = UpdateContext::from_inner(parameters.clone());
    C::update(&mut cx).await.unwrap();
    get_chart_and_assert_eq::<C>(
        &db,
        &expected,
        None,
        None,
        None,
        approximate_trailing_points,
    )
    .await;

    parameters.force_full = false;
    let mut cx = UpdateContext::from_inner(parameters);
    C::update(&mut cx).await.unwrap();
    get_chart_and_assert_eq::<C>(
        &db,
        &expected,
        None,
        None,
        None,
        approximate_trailing_points,
    )
    .await;
}

pub async fn ranged_test_chart<C: ChartUpdater>(
    test_name: &str,
    expected: Vec<(&str, &str)>,
    from: NaiveDate,
    to: NaiveDate,
) {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all(test_name).await;
    let current_time = DateTime::from_str("2023-03-01T12:00:00Z").unwrap();
    let current_date = current_time.date_naive();
    // C::create(&db).await.unwrap();
    fill_mock_blockscout_data(&blockscout, current_date).await;
    let policy = C::missing_date_policy();
    let approximate_trailing_points = C::approximate_trailing_points();

    let mut parameters = UpdateParameters {
        db: &db,
        blockscout: &blockscout,
        current_time,
        force_full: true,
    };
    let mut cx = UpdateContext::from_inner(parameters.clone());
    C::update(&mut cx).await.unwrap();
    get_chart_and_assert_eq::<C>(
        &db,
        &expected,
        Some(from),
        Some(to),
        Some(policy),
        approximate_trailing_points,
    )
    .await;

    parameters.force_full = false;
    let mut cx = UpdateContext::from_inner(parameters);
    C::update(&mut cx).await.unwrap();
    get_chart_and_assert_eq::<C>(
        &db,
        &expected,
        Some(from),
        Some(to),
        Some(policy),
        approximate_trailing_points,
    )
    .await;
}

async fn get_chart_and_assert_eq<C: Chart>(
    db: &DatabaseConnection,
    expected: &Vec<(&str, &str)>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    policy: Option<MissingDatePolicy>,
    approximate_trailing_points: u64,
) {
    let data = get_chart_data(
        db,
        C::name(),
        from,
        to,
        None,
        policy,
        approximate_trailing_points,
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

pub async fn simple_test_counter<C: ChartUpdater>(test_name: &str, expected: &str) {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all(test_name).await;
    let current_time = chrono::DateTime::from_str("2023-03-01T12:00:00Z").unwrap();
    let current_date = current_time.date_naive();

    // C::create(&db).await.unwrap();
    fill_mock_blockscout_data(&blockscout, current_date).await;

    let mut parameters = UpdateParameters {
        db: &db,
        blockscout: &blockscout,
        current_time,
        force_full: true,
    };
    let mut cx = UpdateContext::from_inner(parameters.clone());
    C::update(&mut cx).await.unwrap();
    get_counter_and_assert_eq::<C>(&db, expected).await;

    parameters.force_full = false;
    let mut cx = UpdateContext::from_inner(parameters.clone());
    C::update(&mut cx).await.unwrap();
    get_counter_and_assert_eq::<C>(&db, expected).await;
}

async fn get_counter_and_assert_eq<C: Chart>(db: &DatabaseConnection, expected: &str) {
    let data = get_counters(db).await.unwrap();
    let data = &data[C::name()];
    let value = &data.value;
    assert_eq!(expected, value);
}
