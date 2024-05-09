use super::NewContracts;
use crate::{
    charts::{
        chart::Chart,
        data_source::{UpdateContext, UpdateParameters},
        db_interaction::{
            chart_updaters::{parse_and_cumsum, ChartDependentUpdater, ChartUpdater},
            types::DateValue,
        },
    },
    MissingDatePolicy, UpdateError,
};
use entity::sea_orm_active_enums::ChartType;

use std::marker::PhantomData;

#[derive(Debug, Default)]
pub struct ContractsGrowth {
    parent: PhantomData<NewContracts>,
}

impl ContractsGrowth {
    pub fn new(parent: PhantomData<NewContracts>) -> Self {
        Self { parent }
    }
}

impl ChartDependentUpdater<NewContracts> for ContractsGrowth {
    async fn get_values(parent_data: Vec<DateValue>) -> Result<Vec<DateValue>, UpdateError> {
        parse_and_cumsum::<i64>(parent_data, NewContracts::name())
    }
}

impl crate::Chart for ContractsGrowth {
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

impl ChartUpdater for ContractsGrowth {
    async fn update_values(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        Self::update_with_values(cx).await
    }
}

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
