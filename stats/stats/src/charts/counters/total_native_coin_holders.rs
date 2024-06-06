use crate::data_source::kinds::updateable_chart::last_point::LastPointChartWrapper;

/// Items in this module are not intended to be used outside. They are only public
/// since the actual public type is just an alias (to wrapper).
///
/// I.e. use [`super`]'s types.
pub mod _inner {
    use crate::{
        data_source::kinds::updateable_chart::last_point::LastPointChart,
        lines::NativeCoinHoldersGrowth, Chart, Named,
    };
    use entity::sea_orm_active_enums::ChartType;

    pub struct TotalNativeCoinHoldersInner;

    impl Named for TotalNativeCoinHoldersInner {
        const NAME: &'static str = "totalNativeCoinHolders";
    }

    impl Chart for TotalNativeCoinHoldersInner {
        fn chart_type() -> ChartType {
            ChartType::Counter
        }
    }

    impl LastPointChart for TotalNativeCoinHoldersInner {
        type InnerSource = NativeCoinHoldersGrowth;
    }
}

pub type TotalNativeCoinHolders = LastPointChartWrapper<_inner::TotalNativeCoinHoldersInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_native_coin_holders() {
        simple_test_counter::<TotalNativeCoinHolders>(
            "update_total_native_coin_holders",
            "7",
            None,
        )
        .await;
    }
}
