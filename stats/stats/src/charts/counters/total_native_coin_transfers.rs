use crate::{
    charts::db_interaction::write::insert_data_many,
    data_processing::parse_and_sum,
    data_source::{
        kinds::updateable_chart::{UpdateableChart, UpdateableChartWrapper},
        DataSource, UpdateContext,
    },
    lines::NewNativeCoinTransfers,
    Chart, DateValueString, Named, UpdateError,
};
use blockscout_metrics_tools::AggregateTimer;
use entity::sea_orm_active_enums::ChartType;

pub struct TotalNativeCoinTransfersInner;

impl Named for TotalNativeCoinTransfersInner {
    const NAME: &'static str = "totalNativeCoinTransfers";
}

impl Chart for TotalNativeCoinTransfersInner {
    fn chart_type() -> ChartType {
        ChartType::Counter
    }
}

impl UpdateableChart for TotalNativeCoinTransfersInner {
    type PrimaryDependency = NewNativeCoinTransfers;
    type SecondaryDependencies = ();

    async fn update_values(
        cx: &UpdateContext<'_>,
        chart_id: i32,
        _last_accurate_point: Option<DateValueString>,
        min_blockscout_block: i64,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), UpdateError> {
        let full_data = Self::PrimaryDependency::query_data(cx, None, remote_fetch_timer).await?;
        let sum = parse_and_sum::<i64>(full_data, Self::NAME, Self::PrimaryDependency::NAME)?;
        let Some(sum) = sum else {
            tracing::warn!(
                chart = Self::NAME,
                "dependency did not return any points; skipping the update"
            );
            return Ok(());
        };
        let sum = sum.active_model(chart_id, Some(min_blockscout_block));
        insert_data_many(cx.db, vec![sum])
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(())
    }
}

pub type TotalNativeCoinTransfers = UpdateableChartWrapper<TotalNativeCoinTransfersInner>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_native_coin_transfers() {
        simple_test_counter::<TotalNativeCoinTransfers>("update_total_native_coin_transfers", "17")
            .await;
    }
}
