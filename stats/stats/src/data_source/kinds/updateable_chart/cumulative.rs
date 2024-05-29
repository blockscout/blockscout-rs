//! Chart that accumulates values of another chart.
//!
//! For example, chart "Total accounts" is a cumulative
//! of "New accounts".
//!
//! So, if the values of `NewItemsChart` are [1, 2, 3, 4], then
//! cumulative chart will produce [1, 3, 6, 10].

use std::marker::PhantomData;

use blockscout_metrics_tools::AggregateTimer;
use chrono::Days;

use crate::{
    charts::{
        chart::{chart_portrait, ChartData},
        db_interaction::write::insert_data_many,
    },
    data_processing::parse_and_cumsum,
    data_source::{DataSource, UpdateContext},
    utils::day_start,
    Chart, DateValue, Named, UpdateError,
};

use super::{UpdateableChart, UpdateableChartDataSourceWrapper};

/// See [module-level documentation](self) for details.
pub trait CumulativeChart: Chart {
    type NewItemsChart: DataSource<Output = ChartData> + Named;
}

/// Wrapper struct used for avoiding implementation conflicts
///
/// See [module-level documentation](self) for details.
pub type CumulativeDataSourceWrapper<T> =
    UpdateableChartDataSourceWrapper<CumulativeChartWrapper<T>>;

pub struct CumulativeChartWrapper<T: CumulativeChart>(PhantomData<T>);

impl<T: CumulativeChart + Named> Named for CumulativeChartWrapper<T> {
    const NAME: &'static str = T::NAME;
}

#[portrait::fill(portrait::delegate(T))]
impl<T: CumulativeChart + Chart> Chart for CumulativeChartWrapper<T> {}

impl<T: CumulativeChart> UpdateableChart for CumulativeChartWrapper<T> {
    type PrimaryDependency = T::NewItemsChart;
    type SecondaryDependencies = ();

    async fn update_values(
        cx: &UpdateContext<'_>,
        chart_id: i32,
        last_accurate_point: Option<DateValue>,
        min_blockscout_block: i64,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), UpdateError> {
        let range = last_accurate_point
            .clone()
            .map(|p| day_start(p.date + Days::new(1))..=cx.time);
        let new_accounts =
            Self::PrimaryDependency::query_data(cx, range, remote_fetch_timer).await?;
        let partial_sum = last_accurate_point
            .map(|p| {
                p.value.parse::<i64>().map_err(|e| {
                    UpdateError::Internal(format!(
                        "failed to parse value in chart '{}': {e}",
                        <Self as Named>::NAME
                    ))
                })
            })
            .transpose()?;
        let partial_sum = partial_sum.unwrap_or(0);
        let data = parse_and_cumsum(
            new_accounts.values,
            Self::PrimaryDependency::NAME,
            partial_sum,
        )?
        .into_iter()
        .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));
        insert_data_many(cx.db, data)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(())
    }
}
