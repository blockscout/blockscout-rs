use super::{
    init_db::{init_db_all, init_db_all_multichain},
    mock_blockscout::fill_mock_blockscout_data,
};
use crate::{
    ChartProperties,
    data_source::{
        source::DataSource,
        types::{IndexerMigrations, UpdateContext, UpdateParameters},
    },
    query_dispatch::QuerySerialized,
    range::UniversalRange,
    tests::mock_multichain::fill_mock_multichain_data,
    types::{Timespan, timespans::DateValue},
};
use blockscout_service_launcher::test_database::TestDbGuard;
use chrono::{DateTime, NaiveDateTime, Utc};
use pretty_assertions::assert_eq;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use stats_proto::blockscout::stats::v1::Point;
use std::{fmt::Debug, str::FromStr};

pub fn map_str_tuple_to_owned(l: Vec<(&str, &str)>) -> Vec<(String, String)> {
    l.into_iter()
        .map(|t| (t.0.to_string(), t.1.to_string()))
        .collect()
}

const MIGRATIONS_VARIANTS: [IndexerMigrations; 2] = [
    IndexerMigrations::empty(),
    IndexerMigrations::latest(),
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
    simple_test_chart_inner::<C>(test_name, expected, IndexerMigrations::latest()).await
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

pub fn chart_output_to_expected(output: Vec<Point>) -> Vec<(String, String)> {
    output.into_iter().map(|p| (p.date, p.value)).collect()
}

async fn simple_test_chart_inner<C>(
    test_name: &str,
    expected: Vec<(&str, &str)>,
    migrations: IndexerMigrations,
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
        stats_db: &db,
        is_multichain_mode: false,
        indexer_db: &blockscout,
        indexer_applied_migrations: migrations,
        enabled_update_charts_recursive: C::all_dependencies_chart_keys(),
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
    db: &DatabaseConnection,
    blockscout: &DatabaseConnection,
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
        stats_db: db,
        is_multichain_mode: false,
        indexer_db: blockscout,
        indexer_applied_migrations: IndexerMigrations::latest(),
        enabled_update_charts_recursive: C::all_dependencies_chart_keys(),
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
        IndexerMigrations::latest(),
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
    migrations: IndexerMigrations,
) where
    C: DataSource + ChartProperties + QuerySerialized<Output = Vec<Point>>,
    C::Resolution: Ord + Clone + Debug,
{
    let _ = tracing_subscriber::fmt::try_init();
    let expected = map_str_tuple_to_owned(expected);
    let (db, blockscout) = init_db_all(test_name).await;
    let max_time = DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap();
    let current_time = update_time.map(|t| t.and_utc()).unwrap_or(max_time);
    let max_date = max_time.date_naive();
    let range = { from.into_time_range().start..to.into_time_range().end };
    C::init_recursively(&db, &current_time).await.unwrap();
    fill_mock_blockscout_data(&blockscout, max_date).await;

    let mut parameters = UpdateParameters {
        stats_db: &db,
        is_multichain_mode: false,
        indexer_db: &blockscout,
        indexer_applied_migrations: migrations,
        enabled_update_charts_recursive: C::all_dependencies_chart_keys(),
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
        IndexerMigrations::latest(),
        false,
    )
    .await
}

/// `test_name` must be unique to avoid db clashes
pub async fn simple_test_counter_multichain<C>(
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
        IndexerMigrations::latest(),
        true,
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
        simple_test_counter_inner::<C>(&test_name, expected, update_time, migrations, false).await
    }
}

async fn simple_test_counter_inner<C>(
    test_name: &str,
    expected: &str,
    update_time: Option<NaiveDateTime>,
    migrations: IndexerMigrations,
    multichain_mode: bool,
) where
    C: DataSource + ChartProperties + QuerySerialized<Output = DateValue<String>>,
{
    let max_time = DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap();
    let max_date = max_time.date_naive();
    let (current_time, db, indexer) = if multichain_mode {
        let (t, db, multichain) = prepare_multichain_chart_test::<C>(test_name, update_time).await;
        fill_mock_multichain_data(&multichain, max_date).await;
        (t, db, multichain)
    } else {
        let (t, db, blockscout) = prepare_chart_test::<C>(test_name, update_time).await;
        fill_mock_blockscout_data(&blockscout, max_date).await;
        (t, db, blockscout)
    };

    let mut parameters = UpdateParameters {
        stats_db: &db,
        is_multichain_mode: multichain_mode,
        indexer_db: &indexer,
        indexer_applied_migrations: migrations,
        enabled_update_charts_recursive: C::all_dependencies_chart_keys(),
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

/// Test that the counter returns non-zero fallback value when both
/// - Blockscout data is populated
/// - Update is not called on the counter
pub async fn test_counter_fallback<C>(test_name: &str)
where
    C: DataSource + ChartProperties + QuerySerialized<Output = DateValue<String>>,
{
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all(test_name).await;
    let current_time = chrono::DateTime::from_str("2023-03-01T12:00:00Z").unwrap();
    let current_date = current_time.date_naive();

    C::init_recursively(&db, &current_time).await.unwrap();

    fill_mock_blockscout_data(&blockscout, current_date).await;

    // need to analyze or vacuum for `reltuples` to be updated.
    // source: https://www.postgresql.org/docs/9.3/planner-stats.html
    let _ = blockscout
        .execute(Statement::from_string(DbBackend::Postgres, "ANALYZE;"))
        .await
        .unwrap();

    let parameters = UpdateParameters {
        stats_db: &db,
        is_multichain_mode: false,
        indexer_db: &blockscout,
        indexer_applied_migrations: IndexerMigrations::latest(),
        enabled_update_charts_recursive: C::all_dependencies_chart_keys(),
        update_time_override: Some(current_time),
        force_full: false,
    };
    let cx: UpdateContext<'_> = UpdateContext::from_params_now_or_override(parameters.clone());
    let data = get_counter::<C>(&cx).await;
    assert_ne!("0", data.value);
}

pub async fn prepare_chart_test<C: DataSource + ChartProperties>(
    test_name: &str,
    init_time: Option<NaiveDateTime>,
) -> (DateTime<Utc>, TestDbGuard, TestDbGuard) {
    prepare_chart_test_inner::<C>(test_name, init_time, false).await
}

pub async fn prepare_multichain_chart_test<C: DataSource + ChartProperties>(
    test_name: &str,
    init_time: Option<NaiveDateTime>,
) -> (DateTime<Utc>, TestDbGuard, TestDbGuard) {
    prepare_chart_test_inner::<C>(test_name, init_time, true).await
}

async fn prepare_chart_test_inner<C: DataSource + ChartProperties>(
    test_name: &str,
    init_time: Option<NaiveDateTime>,
    multichain_mode: bool,
) -> (DateTime<Utc>, TestDbGuard, TestDbGuard) {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, indexer) = if multichain_mode {
        init_db_all_multichain(test_name).await
    } else {
        init_db_all(test_name).await
    };
    let init_time = init_time
        .map(|t| t.and_utc())
        .unwrap_or(DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap());
    C::init_recursively(&db, &init_time).await.unwrap();
    (init_time, db, indexer)
}

pub async fn get_counter<C: QuerySerialized<Output = DateValue<String>>>(
    cx: &UpdateContext<'_>,
) -> DateValue<String> {
    C::query_data_static(cx, UniversalRange::full(), None, false)
        .await
        .unwrap()
}
