use super::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data};
use crate::{
    data_source::{
        source::DataSource,
        types::{UpdateContext, UpdateParameters},
    },
    get_line_chart_data, get_raw_counters, ChartProperties, MissingDatePolicy,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use pretty_assertions::assert_eq;
use sea_orm::DatabaseConnection;
use std::str::FromStr;

fn map_str_tuple_to_owned(l: Vec<(&str, &str)>) -> Vec<(String, String)> {
    l.into_iter()
        .map(|t| (t.0.to_string(), t.1.to_string()))
        .collect()
}

/// `test_name` must be unique to avoid db clashes
pub async fn simple_test_chart<C: DataSource + ChartProperties>(
    test_name: &str,
    expected: Vec<(&str, &str)>,
) {
    let _ = tracing_subscriber::fmt::try_init();
    let expected = map_str_tuple_to_owned(expected);
    let (db, blockscout) = init_db_all(test_name).await;
    let current_time = DateTime::from_str("2023-03-01T12:00:00Z").unwrap();
    let current_date = current_time.date_naive();
    C::init_recursively(&db, &current_time).await.unwrap();
    fill_mock_blockscout_data(&blockscout, current_date).await;
    let approximate_trailing_points = C::approximate_trailing_points();

    let mut parameters = UpdateParameters {
        db: &db,
        blockscout: &blockscout,
        update_time_override: Some(current_time),
        force_full: true,
    };
    let cx = UpdateContext::from_params_now_or_override(parameters.clone());
    C::update_recursively(&cx).await.unwrap();
    assert_eq!(
        &get_chart::<C>(
            &db,
            None,
            None,
            C::missing_date_policy(),
            false,
            approximate_trailing_points,
        )
        .await,
        &expected
    );

    parameters.force_full = false;
    let cx = UpdateContext::from_params_now_or_override(parameters);
    C::update_recursively(&cx).await.unwrap();
    assert_eq!(
        &get_chart::<C>(
            &db,
            None,
            None,
            C::missing_date_policy(),
            false,
            approximate_trailing_points,
        )
        .await,
        &expected
    );
}

/// `test_name` must be unique to avoid db clashes
pub async fn ranged_test_chart<C: DataSource + ChartProperties>(
    test_name: &str,
    expected: Vec<(&str, &str)>,
    from: NaiveDate,
    to: NaiveDate,
    update_time: Option<NaiveDateTime>,
) {
    let _ = tracing_subscriber::fmt::try_init();
    let expected = map_str_tuple_to_owned(expected);
    let (db, blockscout) = init_db_all(test_name).await;
    let max_time = DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap();
    let current_time = update_time.map(|t| t.and_utc()).unwrap_or(max_time);
    let max_date = max_time.date_naive();
    C::init_recursively(&db, &current_time).await.unwrap();
    fill_mock_blockscout_data(&blockscout, max_date).await;
    let policy = C::missing_date_policy();
    let approximate_trailing_points = C::approximate_trailing_points();

    let mut parameters = UpdateParameters {
        db: &db,
        blockscout: &blockscout,
        update_time_override: Some(current_time),
        force_full: true,
    };
    let cx = UpdateContext::from_params_now_or_override(parameters.clone());
    C::update_recursively(&cx).await.unwrap();
    assert_eq!(
        &get_chart::<C>(
            &db,
            Some(from),
            Some(to),
            policy,
            false,
            approximate_trailing_points,
        )
        .await,
        &expected
    );

    parameters.force_full = false;
    let cx = UpdateContext::from_params_now_or_override(parameters);
    C::update_recursively(&cx).await.unwrap();
    assert_eq!(
        &get_chart::<C>(
            &db,
            Some(from),
            Some(to),
            policy,
            false,
            approximate_trailing_points,
        )
        .await,
        &expected
    );
}

async fn get_chart<C: DataSource + ChartProperties>(
    db: &DatabaseConnection,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    policy: MissingDatePolicy,
    fill_missing_dates: bool,
    approximate_trailing_points: u64,
) -> Vec<(String, String)> {
    let data = get_line_chart_data(
        db,
        C::NAME,
        from,
        to,
        None,
        policy,
        fill_missing_dates,
        approximate_trailing_points,
    )
    .await
    .unwrap();
    data.into_iter()
        .map(|p| (p.date.to_string(), p.value))
        .collect()
}

/// `test_name` must be unique to avoid db clashes
pub async fn simple_test_counter<C: DataSource + ChartProperties>(
    test_name: &str,
    expected: &str,
    update_time: Option<NaiveDateTime>,
) {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all(test_name).await;
    let max_time = DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap();
    let current_time = update_time.map(|t| t.and_utc()).unwrap_or(max_time);
    let max_date = max_time.date_naive();

    C::init_recursively(&db, &current_time).await.unwrap();
    fill_mock_blockscout_data(&blockscout, max_date).await;

    let mut parameters = UpdateParameters {
        db: &db,
        blockscout: &blockscout,
        update_time_override: Some(current_time),
        force_full: true,
    };
    let cx = UpdateContext::from_params_now_or_override(parameters.clone());
    C::update_recursively(&cx).await.unwrap();
    assert_eq!(expected, get_counter::<C>(&db).await);
    parameters.force_full = false;
    let cx = UpdateContext::from_params_now_or_override(parameters.clone());
    C::update_recursively(&cx).await.unwrap();
    assert_eq!(expected, get_counter::<C>(&db).await);
}

async fn get_counter<C: ChartProperties>(db: &DatabaseConnection) -> String {
    let data = get_raw_counters(db).await.unwrap();
    let data = &data[C::NAME];
    data.value.clone()
}
