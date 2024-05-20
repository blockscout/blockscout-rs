use super::NewContracts;
use crate::{
    charts::{
        chart::Chart,
        db_interaction::{chart_updaters::parse_and_cumsum, write::insert_data_many},
    },
    data_source::{
        kinds::chart::{BatchUpdateableChart, BatchUpdateableChartWrapper, UpdateableChartWrapper},
        source::DataSource,
    },
    MissingDatePolicy, UpdateError,
};
use chrono::Utc;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::DatabaseConnection;

pub struct ContractsGrowthInner;

impl crate::Chart for ContractsGrowthInner {
    const NAME: &'static str = "contractsGrowth";
    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

impl BatchUpdateableChart for ContractsGrowthInner {
    type PrimaryDependency = NewContracts;
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

pub type ContractsGrowth =
    UpdateableChartWrapper<BatchUpdateableChartWrapper<ContractsGrowthInner>>;

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::tests::simple_test::simple_test_chart;

//     #[tokio::test]
//     #[ignore = "needs database to run"]
//     async fn update_contracts_growth() {
//         simple_test_chart::<ContractsGrowth>(
//             "update_contracts_growth",
//             vec![
//                 ("2022-11-09", "3"),
//                 ("2022-11-10", "9"),
//                 ("2022-11-11", "17"),
//                 ("2022-11-12", "19"),
//                 ("2022-12-01", "21"),
//                 ("2023-01-01", "22"),
//                 ("2023-02-01", "23"),
//             ],
//         )
//         .await;
//     }
// }
