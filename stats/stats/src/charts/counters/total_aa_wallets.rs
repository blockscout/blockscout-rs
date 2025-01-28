use crate::{
    data_source::kinds::{
        data_manipulation::{last_point::LastPoint, map::StripExt},
        local_db::DirectPointLocalDbChartSource,
    },
    lines::AccountAbstractionWalletsGrowth,
    ChartProperties, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalAccountAbstractionWallets".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type TotalAccountAbstractionWallets =
    DirectPointLocalDbChartSource<LastPoint<StripExt<AccountAbstractionWalletsGrowth>>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_account_abstraction_wallets() {
        simple_test_counter::<TotalAccountAbstractionWallets>(
            "update_total_account_abstraction_wallets",
            "1",
            None,
        )
        .await;
    }
}
