use std::{collections::HashSet, str::FromStr, sync::Arc};

use chrono::{NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};
use tokio::sync::Mutex;

use crate::{
    charts::db_interaction::{
        chart_updaters::{parse_and_cumsum, RemoteBatchQuery},
        write::insert_data_many,
    },
    construct_update_group,
    tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
    Chart, MissingDatePolicy, UpdateError,
};

use super::{
    group::{SyncUpdateGroup, UpdateGroup},
    kinds::chart::{
        BatchUpdateableChart, BatchUpdateableChartWrapper, RemoteChart, RemoteChartWrapper,
        UpdateableChartWrapper,
    },
    source::DataSource,
    types::UpdateParameters,
};

pub struct NewContractsChart;

impl crate::Chart for NewContractsChart {
    const NAME: &'static str = "newContracts";

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type NewContractsChartSource =
    UpdateableChartWrapper<BatchUpdateableChartWrapper<RemoteChartWrapper<NewContractsChart>>>;

pub struct NewContractsRemote;

impl RemoteBatchQuery for NewContractsRemote {
    fn get_query(from: NaiveDate, to: NaiveDate) -> Statement {
        Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"SELECT day AS date, COUNT(*)::text AS value
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
                        b.timestamp != to_timestamp(0) AND
                        b.timestamp::date < $2 AND
                        b.timestamp::date >= $1
                    UNION
                    SELECT
                        it.created_contract_address_hash AS hash,
                        b.timestamp::date AS day
                    FROM internal_transactions it
                        JOIN blocks b ON b.hash = it.block_hash
                    WHERE
                        it.created_contract_address_hash NOTNULL AND
                        b.consensus = TRUE AND
                        b.timestamp != to_timestamp(0) AND
                        b.timestamp::date < $2 AND
                        b.timestamp::date >= $1
                ) txns_plus_internal_txns
            ) sub
            GROUP BY sub.day;
            "#,
            vec![from.into(), to.into()],
        )
    }
}

impl RemoteChart for NewContractsChart {
    type Dependency = NewContractsRemote;
}

pub struct ContractsGrowthChart;

impl Chart for ContractsGrowthChart {
    const NAME: &'static str = "contractsGrowth";

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

impl BatchUpdateableChart for ContractsGrowthChart {
    type PrimaryDependency = NewContractsChartSource;
    type SecondaryDependencies = ();

    fn step_duration() -> chrono::Duration {
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
        let found = primary_data.values.len();
        let values = parse_and_cumsum::<i64>(primary_data.values, Self::PrimaryDependency::NAME)?
            .into_iter()
            .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(found)
    }
}

pub type ContractsGrowthChartSource =
    UpdateableChartWrapper<BatchUpdateableChartWrapper<ContractsGrowthChart>>;

construct_update_group!(ExampleUpdateGroup {
    name: "exampleGroup",
    charts: [NewContractsChartSource, ContractsGrowthChartSource],
});

#[tokio::test]
#[ignore = "needs database to run"]
async fn _update_examples() {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all("update_examples").await;
    let current_time = chrono::DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap();
    let current_date = current_time.date_naive();
    fill_mock_blockscout_data(&blockscout, current_date).await;
    let enabled =
        HashSet::from([NewContractsChart::NAME, ContractsGrowthChart::NAME].map(|l| l.to_owned()));
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
