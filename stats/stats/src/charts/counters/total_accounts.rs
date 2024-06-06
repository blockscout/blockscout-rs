use crate::data_source::kinds::updateable_chart::last_point::LastPointChartWrapper;

/// Items in this module are not intended to be used outside. They are only public
/// since the actual public type is just an alias (to wrapper).
///
/// I.e. use [`super`]'s types.
pub mod _inner {
    use crate::{
        data_source::kinds::updateable_chart::last_point::LastPointChart, lines::AccountsGrowth,
        Chart, Named,
    };
    use entity::sea_orm_active_enums::ChartType;

    pub struct TotalAccountsInner;

    impl Named for TotalAccountsInner {
        const NAME: &'static str = "totalAccounts";
    }

    impl Chart for TotalAccountsInner {
        fn chart_type() -> ChartType {
            ChartType::Counter
        }
    }

    impl LastPointChart for TotalAccountsInner {
        type InnerSource = AccountsGrowth;
    }
}

pub type TotalAccounts = LastPointChartWrapper<_inner::TotalAccountsInner>;

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
