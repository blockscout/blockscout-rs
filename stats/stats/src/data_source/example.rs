use std::{collections::HashSet, ops::RangeInclusive, str::FromStr, sync::Arc};

use chrono::Utc;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};
use tokio::sync::Mutex;

use super::{
    kinds::{
        remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        updateable_chart::{
            batch::{BatchChart, BatchChartWrapper},
            clone::{CloneChart, CloneChartWrapper},
        },
    },
    source::DataSource,
    types::UpdateParameters,
};
use crate::{
    charts::db_interaction::write::insert_data_many,
    construct_update_group,
    data_processing::parse_and_cumsum,
    tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
    update_group::{SyncUpdateGroup, UpdateGroup},
    utils::sql_with_range_filter_opt,
    Chart, DateValueString, MissingDatePolicy, Named, UpdateError,
};

pub struct NewContractsQuery;

impl StatementFromRange for NewContractsQuery {
    fn get_statement(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT day AS date, COUNT(*)::text AS value
                FROM (
                    SELECT
                        DISTINCT ON (txns_plus_internal_txns.hash)
                        txns_plus_internal_txns.day
                    FROM (
                        SELECT
                            t.created_contract_address_hash AS hash,
                            b.timestamp::date AS day
                        FROM transactions t
                            JOIN blocks b ON b.hash = t.block_hash
                        WHERE
                            t.created_contract_address_hash NOTNULL AND
                            b.consensus = TRUE AND
                            b.timestamp != to_timestamp(0) {filter}
                        UNION
                        SELECT
                            it.created_contract_address_hash AS hash,
                            b.timestamp::date AS day
                        FROM internal_transactions it
                            JOIN blocks b ON b.hash = it.block_hash
                        WHERE
                            it.created_contract_address_hash NOTNULL AND
                            b.consensus = TRUE AND
                            b.timestamp != to_timestamp(0) {filter}
                    ) txns_plus_internal_txns
                ) sub
                GROUP BY sub.day;
            "#,
            [],
            "b.timestamp",
            range
        )
    }
}

pub type NewContractsRemote =
    RemoteDatabaseSource<PullAllWithAndSort<NewContractsQuery, DateValueString>>;

pub struct NewContractsChart;

impl Named for NewContractsChart {
    const NAME: &'static str = "newContracts";
}

impl Chart for NewContractsChart {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

// Directly uses results of SQL query (from `NewContractsRemote`),
// thus `CloneChart`.
impl CloneChart for NewContractsChart {
    type Dependency = NewContractsRemote;
}

// Wrap the earth out of it to obtain `DataSource`-implementing type.
// `Chart` implementation is propageted through the wrappers.
pub type NewContracts = CloneChartWrapper<NewContractsChart>;

pub struct ContractsGrowthChart;

impl Named for ContractsGrowthChart {
    const NAME: &'static str = "contractsGrowth";
}

impl Chart for ContractsGrowthChart {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

// We want to do some custom logic based on data from `NewContracts`.
// However, batch logic fits this dependency. Therefore, `BatchChart`.
impl BatchChart for ContractsGrowthChart {
    type PrimaryDependency = NewContracts;
    type SecondaryDependencies = ();
    type Point = DateValueString;

    fn batch_len() -> chrono::Duration {
        // we need to count cumulative from the beginning
        chrono::Duration::max_value()
    }

    async fn batch_update_values_step_with(
        db: &DatabaseConnection,
        chart_id: i32,
        _update_time: chrono::DateTime<Utc>,
        min_blockscout_block: i64,
        primary_data: <Self::PrimaryDependency as DataSource>::Output,
        _secondary_data: <Self::SecondaryDependencies as DataSource>::Output,
    ) -> Result<usize, UpdateError> {
        let found = primary_data.len();
        let values = parse_and_cumsum::<i64>(primary_data, Self::PrimaryDependency::NAME, 0)?
            .into_iter()
            .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(found)
    }
}

pub type ContractsGrowth = BatchChartWrapper<ContractsGrowthChart>;

// Put the data sources into the group
construct_update_group!(ExampleUpdateGroup {
    name: "exampleGroup",
    charts: [NewContracts, ContractsGrowth],
});

#[tokio::test]
#[ignore = "needs database to run"]
async fn update_examples() {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all("update_examples").await;
    let current_time = chrono::DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap();
    let current_date = current_time.date_naive();
    fill_mock_blockscout_data(&blockscout, current_date).await;
    let enabled =
        HashSet::from([NewContractsChart::NAME, ContractsGrowthChart::NAME].map(|l| l.to_owned()));

    // In this case plain `ExampleUpdateGroup` would suffice, but the example
    // shows what to do in case of >1 groups (to keep it concise there's no 2nd group)

    // Since we want sync group, we need mutexes for each chart
    let mutexes = ExampleUpdateGroup
        .list_dependency_mutex_ids()
        .into_iter()
        .map(|id| (id.to_owned(), Arc::new(Mutex::new(()))))
        .collect();
    let group = SyncUpdateGroup::new(&mutexes, Arc::new(ExampleUpdateGroup)).unwrap();
    group
        .create_charts_with_mutexes(&db, None, &enabled)
        .await
        .unwrap();

    let parameters = UpdateParameters {
        db: &db,
        blockscout: &blockscout,
        update_time_override: None,
        force_full: true,
    };
    group
        .update_charts_with_mutexes(parameters, &enabled)
        .await
        .unwrap();
}
