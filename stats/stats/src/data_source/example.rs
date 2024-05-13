use std::{collections::HashSet, str::FromStr};

use chrono::Utc;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;

use crate::{
    charts::db_interaction::{chart_updaters::parse_and_cumsum, write::insert_data_many},
    construct_update_group,
    lines::NewContracts,
    tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
    Chart, MissingDatePolicy, UpdateError,
};

use super::{
    group::UpdateGroup,
    kinds::{
        batch_chart::{BatchUpdateableChart, BatchUpdateableChartWrapper},
        chart::UpdateableChartWrapper,
        remote::RemoteSourceWrapper,
    },
    source_trait::DataSource,
    types::UpdateParameters,
};

struct NewContractsChart;

impl crate::Chart for NewContractsChart {
    fn name() -> &'static str {
        "newContracts"
    }

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

type NewContractsSource = RemoteSourceWrapper<NewContracts>;
type NewContractsChartSource =
    UpdateableChartWrapper<BatchUpdateableChartWrapper<NewContractsChart>>;

impl BatchUpdateableChart for NewContractsChart {
    type PrimaryDependency = NewContractsSource;
    type SecondaryDependencies = ();

    async fn batch_update_values_step_with(
        db: &DatabaseConnection,
        chart_id: i32,
        _update_time: chrono::DateTime<Utc>,
        min_blockscout_block: i64,
        primary_data: <Self::PrimaryDependency as DataSource>::Output,
        _secondary_data: <Self::SecondaryDependencies as DataSource>::Output,
    ) -> Result<usize, UpdateError> {
        let found = primary_data.len();
        let values = primary_data
            .into_iter()
            .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(found)
    }
}

struct ContractsGrowthChart;

impl Chart for ContractsGrowthChart {
    fn name() -> &'static str {
        "contractsGrowth"
    }
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
        let values = parse_and_cumsum::<i64>(primary_data.values, Self::PrimaryDependency::name())?
            .into_iter()
            .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));
        insert_data_many(db, values)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(found)
    }
}

type ContractsGrowthChartSource =
    UpdateableChartWrapper<BatchUpdateableChartWrapper<ContractsGrowthChart>>;

construct_update_group!(ExampleUpdateGroup = [NewContractsChartSource, ContractsGrowthChartSource]);

#[tokio::test]
async fn _update_examples() {
    let _ = tracing_subscriber::fmt::try_init();
    let (db, blockscout) = init_db_all("update_examples").await;
    let current_time = chrono::DateTime::from_str("2023-03-01T12:00:00Z").unwrap();
    let current_date = current_time.date_naive();
    fill_mock_blockscout_data(&blockscout, current_date).await;
    let enabled = HashSet::from(
        [NewContractsChart::name(), ContractsGrowthChart::name()].map(|l| l.to_owned()),
    );
    ExampleUpdateGroup::create_charts(&db, &enabled, &current_time)
        .await
        .unwrap();

    let parameters = UpdateParameters {
        db: &db,
        blockscout: &blockscout,
        current_time,
        force_full: true,
    };
    ExampleUpdateGroup::update_charts(parameters, &enabled)
        .await
        .unwrap();
}
