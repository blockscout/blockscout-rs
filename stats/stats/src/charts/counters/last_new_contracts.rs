use crate::{
    data_source::kinds::{
        data_manipulation::{last_point::LastPoint, map::StripExt},
        local_db::DirectPointLocalDbChartSource,
    },
    lines::NewContracts,
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "lastNewContracts".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
}

pub type LastNewContracts =
    DirectPointLocalDbChartSource<LastPoint<StripExt<NewContracts>>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_last_new_contracts() {
        simple_test_counter::<LastNewContracts>("update_last_new_contracts", "0", None).await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_last_new_contracts_update_before_new() {
        simple_test_counter::<LastNewContracts>(
            "update_last_new_contracts_update_before_new",
            "0",
            Some(dt("2023-02-01T09:00:00")),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_last_new_contracts_update_after_new() {
        simple_test_counter::<LastNewContracts>(
            "update_last_new_contracts_update_after_new",
            "1",
            Some(dt("2023-02-01T10:00:01")),
        )
        .await;
    }
}
