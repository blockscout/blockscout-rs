use super::{
    init_db::{init_db_all, init_marked_db_all},
    mock_blockscout::fill_mock_blockscout_data,
};
use crate::{
    data_source::{
        source::DataSource,
        types::{BlockscoutMigrations, UpdateContext, UpdateParameters},
    },
    query_dispatch::QuerySerialized,
    range::UniversalRange,
    types::{timespans::DateValue, Timespan},
    utils::MarkedDbConnection,
    ChartProperties,
};
use blockscout_service_launcher::test_database::TestDbGuard;
use chrono::{DateTime, NaiveDateTime, Utc};
use pretty_assertions::assert_eq;
use stats_proto::blockscout::stats::v1::Point;
use std::{fmt::Debug, str::FromStr};

pub fn map_str_tuple_to_owned(l: Vec<(&str, &str)>) -> Vec<(String, String)> {
    l.into_iter()
        .map(|t| (t.0.to_string(), t.1.to_string()))
        .collect()
}

const MIGRATIONS_VARIANTS: [BlockscoutMigrations; 2] = [
    BlockscoutMigrations::empty(),
    BlockscoutMigrations::latest(),
];

/// `test_name` must be unique to avoid db clashes
///
/// returns db handles to continue testing if needed
pub async fn simple_test_chart<C>(
    test_name: &str,
    expected: Vec<(&str, &str)>,
) -> (TestDbGuard, TestDbGuard)
where
    C: DataSource + ChartProperties + QuerySerialized<Output = Vec<Point>>,
    C::Resolution: Ord + Clone + Debug,
{
    simple_test_chart_inner::<C>(test_name, expected, BlockscoutMigrations::latest()).await
}

/// tests all statement kinds for different migrations combinations.
/// NOTE: everything is tested with only one version of DB (probably latest),
/// so statements incompatible with the current mock DB setup are going to break.
///
/// - db is going to be initialized separately for each variant
/// - `_N` will be added to `test_name_base` for each variant
/// - the resulting test name must be unique to avoid db clashes
pub async fn simple_test_chart_with_migration_variants<C>(
    test_name_base: &str,
    expected: Vec<(&str, &str)>,
) where
    C: DataSource + ChartProperties + QuerySerialized<Output = Vec<Point>>,
    C::Resolution: Ord + Clone + Debug,
{
    for (i, migrations) in MIGRATIONS_VARIANTS.into_iter().enumerate() {
        let test_name = format!("{test_name_base}_{i}");
        simple_test_chart_inner::<C>(&test_name, expected.clone(), migrations).await;
    }
}

fn chart_output_to_expected(output: Vec<Point>) -> Vec<(String, String)> {
    output.into_iter().map(|p| (p.date, p.value)).collect()
}

async fn simple_test_chart_inner<C>(
    test_name: &str,
    expected: Vec<(&str, &str)>,
    migrations: BlockscoutMigrations,
) -> (TestDbGuard, TestDbGuard)
where
    C: DataSource + ChartProperties + QuerySerialized<Output = Vec<Point>>,
    C::Resolution: Ord + Clone + Debug,
{
    let (current_time, db, blockscout) = prepare_chart_test::<C>(test_name, None).await;
    let expected = map_str_tuple_to_owned(expected);
    let current_date = current_time.date_naive();
    fill_mock_blockscout_data(&blockscout, current_date).await;

    let mut parameters = UpdateParameters {
        db: &MarkedDbConnection::from_test_db(&db).unwrap(),
        blockscout: &MarkedDbConnection::from_test_db(&blockscout).unwrap(),
        blockscout_applied_migrations: migrations,
        update_time_override: Some(current_time),
        force_full: true,
    };
    let cx = UpdateContext::from_params_now_or_override(parameters.clone());
    C::update_recursively(&cx).await.unwrap();
    assert_eq!(
        &chart_output_to_expected(
            C::query_data_static(&cx, UniversalRange::full(), None, false)
                .await
                .unwrap()
        ),
        &expected
    );

    parameters.force_full = false;
    let cx = UpdateContext::from_params_now_or_override(parameters);
    C::update_recursively(&cx).await.unwrap();
    assert_eq!(
        &chart_output_to_expected(
            C::query_data_static(&cx, UniversalRange::full(), None, false)
                .await
                .unwrap()
        ),
        &expected
    );
    (db, blockscout)
}

/// Expects to have `test_name` db's initialized (e.g. by [`simple_test_chart`]).
///
/// Tests that force update with existing data works correctly
pub async fn dirty_force_update_and_check<C>(
    db: &TestDbGuard,
    blockscout: &TestDbGuard,
    expected: Vec<(&str, &str)>,
    update_time_override: Option<DateTime<Utc>>,
) where
    C: DataSource + ChartProperties + QuerySerialized<Output = Vec<Point>>,
    C::Resolution: Ord + Clone + Debug,
{
    let _ = tracing_subscriber::fmt::try_init();
    let expected = map_str_tuple_to_owned(expected);
    // some later time so that the update is not skipped
    let current_time =
        update_time_override.unwrap_or(DateTime::from_str("2023-03-01T12:00:01Z").unwrap());

    let parameters = UpdateParameters {
        db: &MarkedDbConnection::from_test_db(db).unwrap(),
        blockscout: &MarkedDbConnection::from_test_db(blockscout).unwrap(),
        blockscout_applied_migrations: BlockscoutMigrations::latest(),
        update_time_override: Some(current_time),
        force_full: true,
    };
    let cx = UpdateContext::from_params_now_or_override(parameters.clone());
    C::update_recursively(&cx).await.unwrap();
    assert_eq!(
        &chart_output_to_expected(
            C::query_data_static(&cx, UniversalRange::full(), None, false)
                .await
                .unwrap()
        ),
        &expected
    );
}

/// tests only case with all migrations applied, to
/// test statements for different migrations combinations
/// use [`ranged_test_chart_with_migration_variants`]
///
/// - db is going to be initialized separately for each variant
/// - `_N` will be added to `test_name_base` for each variant
/// - the resulting test name must be unique to avoid db clashes
pub async fn ranged_test_chart<C>(
    test_name: &str,
    expected: Vec<(&str, &str)>,
    from: C::Resolution,
    to: C::Resolution,
    update_time: Option<NaiveDateTime>,
) where
    C: DataSource + ChartProperties + QuerySerialized<Output = Vec<Point>>,
    C::Resolution: Ord + Clone + Debug,
{
    ranged_test_chart_inner::<C>(
        test_name,
        expected,
        from,
        to,
        update_time,
        BlockscoutMigrations::latest(),
    )
    .await
}

/// tests all statement kinds for different migrations combinations.
/// NOTE: everything is tested with only one version of DB (probably latest),
/// so statements incompatible with the current mock DB setup are going to break.
///
/// - db is going to be initialized separately for each variant
/// - `_N` will be added to `test_name_base` for each variant
/// - the resulting test name must be unique to avoid db clashes
pub async fn ranged_test_chart_with_migration_variants<C>(
    test_name_base: &str,
    expected: Vec<(&str, &str)>,
    from: C::Resolution,
    to: C::Resolution,
    update_time: Option<NaiveDateTime>,
) where
    C: DataSource + ChartProperties + QuerySerialized<Output = Vec<Point>>,
    C::Resolution: Ord + Clone + Debug,
{
    for (i, migrations) in MIGRATIONS_VARIANTS.into_iter().enumerate() {
        let test_name = format!("{test_name_base}_{i}");
        ranged_test_chart_inner::<C>(
            &test_name,
            expected.clone(),
            from.clone(),
            to.clone(),
            update_time,
            migrations,
        )
        .await;
    }
}

async fn ranged_test_chart_inner<C>(
    test_name: &str,
    expected: Vec<(&str, &str)>,
    from: C::Resolution,
    to: C::Resolution,
    update_time: Option<NaiveDateTime>,
    migrations: BlockscoutMigrations,
) where
    C: DataSource + ChartProperties + QuerySerialized<Output = Vec<Point>>,
    C::Resolution: Ord + Clone + Debug,
{
    let _ = tracing_subscriber::fmt::try_init();
    let expected = map_str_tuple_to_owned(expected);
    let (db, blockscout) = init_marked_db_all(test_name).await;
    let max_time = DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap();
    let current_time = update_time.map(|t| t.and_utc()).unwrap_or(max_time);
    let max_date = max_time.date_naive();
    let range = { from.into_time_range().start..to.into_time_range().end };
    C::init_recursively(db.connection.as_ref(), &current_time)
        .await
        .unwrap();
    fill_mock_blockscout_data(blockscout.connection.as_ref(), max_date).await;

    let mut parameters = UpdateParameters {
        db: &db,
        blockscout: &blockscout,
        blockscout_applied_migrations: migrations,
        update_time_override: Some(current_time),
        force_full: true,
    };
    let cx = UpdateContext::from_params_now_or_override(parameters.clone());
    C::update_recursively(&cx).await.unwrap();
    assert_eq!(
        &chart_output_to_expected(
            C::query_data_static(&cx, range.clone().into(), None, false)
                .await
                .unwrap()
        ),
        &expected
    );

    parameters.force_full = false;
    let cx = UpdateContext::from_params_now_or_override(parameters);
    C::update_recursively(&cx).await.unwrap();
    assert_eq!(
        &chart_output_to_expected(
            C::query_data_static(&cx, range.into(), None, false)
                .await
                .unwrap()
        ),
        &expected
    );
}

/// `test_name` must be unique to avoid db clashes
pub async fn simple_test_counter<C>(
    test_name: &str,
    expected: &str,
    update_time: Option<NaiveDateTime>,
) where
    C: DataSource + ChartProperties + QuerySerialized<Output = DateValue<String>>,
{
    simple_test_counter_inner::<C>(
        test_name,
        expected,
        update_time,
        BlockscoutMigrations::latest(),
    )
    .await
}

/// tests all statement kinds for different migrations combinations.
/// NOTE: everything is tested with only one version of DB (probably latest),
/// so statements incompatible with the current mock DB setup are going to break.
///
/// - db is going to be initialized separately for each variant
/// - `_N` will be added to `test_name_base` for each variant
/// - the resulting test name must be unique to avoid db clashes
pub async fn simple_test_counter_with_migration_variants<C>(
    test_name_base: &str,
    expected: &str,
    update_time: Option<NaiveDateTime>,
) where
    C: DataSource + ChartProperties + QuerySerialized<Output = DateValue<String>>,
{
    for (i, migrations) in MIGRATIONS_VARIANTS.into_iter().enumerate() {
        let test_name = format!("{test_name_base}_{i}");
        simple_test_counter_inner::<C>(&test_name, expected, update_time, migrations).await
    }
}

async fn simple_test_counter_inner<C>(
    test_name: &str,
    expected: &str,
    update_time: Option<NaiveDateTime>,
    migrations: BlockscoutMigrations,
) where
    C: DataSource + ChartProperties + QuerySerialized<Output = DateValue<String>>,
{
    let (current_time, db, blockscout) = prepare_chart_test::<C>(test_name, update_time).await;
    let max_time = DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap();
    let max_date = max_time.date_naive();
    fill_mock_blockscout_data(&blockscout, max_date).await;

    let mut parameters = UpdateParameters {
        db: &MarkedDbConnection::from_test_db(&db).unwrap(),
        blockscout: &MarkedDbConnection::from_test_db(&blockscout).unwrap(),
        blockscout_applied_migrations: migrations,
        update_time_override: Some(current_time),
        force_full: true,
    };
    let cx = UpdateContext::from_params_now_or_override(parameters.clone());
    C::update_recursively(&cx).await.unwrap();
    assert_eq!(expected, get_counter::<C>(&cx).await.value);
    parameters.force_full = false;
    let cx = UpdateContext::from_params_now_or_override(parameters.clone());
    C::update_recursively(&cx).await.unwrap();
    assert_eq!(expected, get_counter::<C>(&cx).await.value);
}

pub async fn prepare_chart_test<C: DataSource + ChartProperties>(
    test_name: &str,
    init_time: Option<NaiveDateTime>,
) -> (DateTime<Utc>, TestDbGuard, TestDbGuard) {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all(test_name).await;
    let init_time = init_time
        .map(|t| t.and_utc())
        .unwrap_or(DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap());
    C::init_recursively(&db, &init_time).await.unwrap();
    (init_time, db, blockscout)
}

pub async fn get_counter<C: QuerySerialized<Output = DateValue<String>>>(
    cx: &UpdateContext<'_>,
) -> DateValue<String> {
    C::query_data_static(cx, UniversalRange::full(), None, false)
        .await
        .unwrap()
}
