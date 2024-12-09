use crate::{
    data_source::kinds::{
        data_manipulation::{last_point::LastPoint, map::StripExt},
        local_db::DirectPointLocalDbChartSource,
    },
    lines::NewVerifiedContracts,
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "lastNewVerifiedContracts".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
}

pub type LastNewVerifiedContracts =
    DirectPointLocalDbChartSource<LastPoint<StripExt<NewVerifiedContracts>>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_last_new_verified_contracts() {
        simple_test_counter::<LastNewVerifiedContracts>(
            "update_last_new_verified_contracts",
            "0",
            None,
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_last_new_verified_contracts_update_before_new() {
        simple_test_counter::<LastNewVerifiedContracts>(
            "update_last_new_verified_contracts_update_before_new",
            "0",
            Some(dt("2022-11-15T14:59:59")),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_last_new_verified_contracts_update_after_new() {
        simple_test_counter::<LastNewVerifiedContracts>(
            "update_last_new_verified_contracts_update_after_new",
            "1",
            Some(dt("2022-11-15T15:00:01")),
        )
        .await;
    }
}
