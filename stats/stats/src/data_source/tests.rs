use std::{collections::HashSet, ops::Range, str::FromStr, sync::Arc};

use chrono::{DateTime, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};
use tokio::sync::Mutex;

use super::{
    kinds::{
        data_manipulation::map::MapParseTo,
        local_db::{
            parameters::update::batching::parameter_traits::BatchStepBehaviour,
            BatchLocalDbChartSourceWithDefaultParams, CumulativeLocalDbChartSource,
            DirectVecLocalDbChartSource,
        },
        remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
    },
    types::UpdateParameters,
};
use crate::{
    charts::db_interaction::types::DateValueInt,
    construct_update_group,
    data_source::kinds::local_db::parameters::update::batching::parameters::PassVecStep,
    tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
    update_group::{SyncUpdateGroup, UpdateGroup},
    utils::sql_with_range_filter_opt,
    ChartProperties, DateValueString, MissingDatePolicy, Named, UpdateError,
};

pub struct NewContractsQuery;

impl StatementFromRange for NewContractsQuery {
    fn get_statement(range: Option<Range<DateTimeUtc>>) -> Statement {
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

pub struct NewContractsChartProperties;

impl Named for NewContractsChartProperties {
    const NAME: &'static str = "newContracts";
}

impl ChartProperties for NewContractsChartProperties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

// Directly uses results of SQL query (from `NewContractsRemote`)
pub type NewContracts =
    DirectVecLocalDbChartSource<NewContractsRemote, NewContractsChartProperties>;

pub type NewContractsInt = MapParseTo<NewContracts, DateValueInt>;

pub struct ContractsGrowthProperties;

impl Named for ContractsGrowthProperties {
    const NAME: &'static str = "contractsGrowth";
}

impl ChartProperties for ContractsGrowthProperties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

// We can use convenient common implementation to get growth chart
pub type ContractsGrowth = CumulativeLocalDbChartSource<NewContractsInt, ContractsGrowthProperties>;

// Alternatively, if we wanted to preform some custom logic on each batch step, we can do
#[allow(unused)]
struct ContractsGrowthCustomBatchStepBehaviour;

impl BatchStepBehaviour<Vec<DateValueString>, ()> for ContractsGrowthCustomBatchStepBehaviour {
    async fn batch_update_values_step_with(
        _db: &DatabaseConnection,
        _chart_id: i32,
        _update_time: DateTime<Utc>,
        _min_blockscout_block: i64,
        _last_accurate_point: DateValueString,
        _main_data: Vec<DateValueString>,
        _resolution_data: (),
    ) -> Result<usize, UpdateError> {
        // do something
        todo!();
        // save data
        #[allow(unreachable_code)]
        PassVecStep::batch_update_values_step_with(
            _db,
            _chart_id,
            _update_time,
            _min_blockscout_block,
            _last_accurate_point,
            _main_data,
            _resolution_data,
        )
        .await
    }
}

#[allow(unused)]
type AlternativeContractsGrowth = BatchLocalDbChartSourceWithDefaultParams<
    NewContracts,
    (),
    ContractsGrowthCustomBatchStepBehaviour,
    ContractsGrowthProperties,
>;

// Put the data sources into the group
construct_update_group!(ExampleUpdateGroup {
    charts: [NewContracts, ContractsGrowth],
});

#[tokio::test]
#[ignore = "needs database to run"]
async fn update_examples() {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all("update_examples").await;
    let current_time = DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap();
    let current_date = current_time.date_naive();
    fill_mock_blockscout_data(&blockscout, current_date).await;
    let enabled = HashSet::from(
        [
            NewContractsChartProperties::NAME,
            ContractsGrowthProperties::NAME,
        ]
        .map(|l| l.to_owned()),
    );

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
