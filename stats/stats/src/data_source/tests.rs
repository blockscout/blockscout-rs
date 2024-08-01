use std::{collections::HashSet, ops::Range, str::FromStr, sync::Arc};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};
use tokio::sync::Mutex;

use super::{
    kinds::{
        data_manipulation::{map::MapParseTo, resolutions::last_value::LastValueLowerResolution},
        local_db::{
            parameters::update::batching::{
                parameter_traits::BatchStepBehaviour,
                parameters::{Batch30Days, Batch30Weeks, Batch30Years, Batch36Months},
            },
            BatchLocalDbChartSourceWithDefaultParams, DailyCumulativeLocalDbChartSource,
            DirectVecLocalDbChartSource,
        },
        remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
    },
    types::UpdateParameters,
};
use crate::{
    construct_update_group,
    data_source::kinds::local_db::parameters::update::batching::parameters::PassVecStep,
    delegated_properties_with_resolutions,
    tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
    types::timespans::{DateValue, Month, Week, Year},
    update_group::{SyncUpdateGroup, UpdateGroup},
    utils::sql_with_range_filter_opt,
    ChartProperties, MissingDatePolicy, Named, UpdateError,
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
    RemoteDatabaseSource<PullAllWithAndSort<NewContractsQuery, NaiveDate, String>>;

pub struct NewContractsChartProperties;

impl Named for NewContractsChartProperties {
    fn name() -> String {
        "newContracts".into()
    }
}

impl ChartProperties for NewContractsChartProperties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

// Directly uses results of SQL query (from `NewContractsRemote`)
pub type NewContracts =
    DirectVecLocalDbChartSource<NewContractsRemote, Batch30Days, NewContractsChartProperties>;

pub type NewContractsInt = MapParseTo<NewContracts, i64>;

pub struct ContractsGrowthProperties;

impl Named for ContractsGrowthProperties {
    fn name() -> String {
        "contractsGrowth".into()
    }
}

impl ChartProperties for ContractsGrowthProperties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

// We can use convenient common implementation to get growth chart
delegated_properties_with_resolutions!(
    delegate: {
        ContractsGrowthWeeklyProperties:  Week,
        ContractsGrowthMonthlyProperties: Month,
        ContractsGrowthYearlyProperties: Year,
    }
    ..ContractsGrowthProperties
);

pub type ContractsGrowth =
    DailyCumulativeLocalDbChartSource<NewContractsInt, ContractsGrowthProperties>;
pub type ContractsGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<ContractsGrowth, Week>,
    Batch30Weeks,
    ContractsGrowthWeeklyProperties,
>;
pub type ContractsGrowthMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<ContractsGrowth, Month>,
    Batch36Months,
    ContractsGrowthMonthlyProperties,
>;
pub type ContractsGrowthYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<ContractsGrowth, Year>,
    Batch30Years,
    ContractsGrowthYearlyProperties,
>;

// const B: <NewContracts as DataSource>::Output = 0;

// Alternatively, if we wanted to preform some custom logic on each batch step, we could do
#[allow(unused)]
struct ContractsGrowthCustomBatchStepBehaviour;

impl BatchStepBehaviour<NaiveDate, Vec<DateValue<String>>, ()>
    for ContractsGrowthCustomBatchStepBehaviour
{
    async fn batch_update_values_step_with(
        _db: &DatabaseConnection,
        _chart_id: i32,
        _update_time: DateTime<Utc>,
        _min_blockscout_block: i64,
        _last_accurate_point: DateValue<String>,
        _main_data: Vec<DateValue<String>>,
        _resolution_data: (),
    ) -> Result<usize, UpdateError> {
        // do something (just an example, not intended for running)
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
    charts: [
        NewContracts,
        ContractsGrowth,
        ContractsGrowthWeekly,
        ContractsGrowthMonthly,
        ContractsGrowthYearly,
    ],
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
            NewContractsChartProperties::key(),
            ContractsGrowthProperties::key(),
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
