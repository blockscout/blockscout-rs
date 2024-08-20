use crate::{
    data_source::kinds::{
        data_manipulation::last_point::LastPoint, local_db::DirectPointLocalDbChartSource,
    },
    lines::AccountsGrowth,
    ChartProperties, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalAccounts".into()
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

pub type TotalAccounts = DirectPointLocalDbChartSource<LastPoint<AccountsGrowth>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_accounts() {
        simple_test_counter::<TotalAccounts>("update_total_accounts", "9", None).await;
    }
}
