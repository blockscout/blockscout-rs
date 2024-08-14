use std::{fmt::Debug, marker::PhantomData, ops::Range};

use sea_orm::prelude::DateTimeUtc;

use crate::{
    charts::db_interaction::read::get_counter_data,
    data_source::{kinds::local_db::parameter_traits::QueryBehaviour, UpdateContext},
    get_line_chart_data,
    types::{timespans::DateValue, Timespan, TimespanValue},
    utils::exclusive_datetime_range_to_inclusive,
    ChartProperties, UpdateError,
};

/// Usually the choice for line charts
pub struct DefaultQueryVec<C: ChartProperties>(PhantomData<C>);

impl<C> QueryBehaviour for DefaultQueryVec<C>
where
    C: ChartProperties,
    C::Resolution: Timespan + Ord + Debug + Clone + Send,
{
    type Output = Vec<TimespanValue<C::Resolution, String>>;

    /// Retrieve chart data from local storage.
    ///
    /// Note that the data might have missing points for efficiency reasons.
    ///
    /// Expects metadata to be consistent with stored data
    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTimeUtc>>,
    ) -> Result<Self::Output, UpdateError> {
        // In DB we store data with date precision. Also, `get_line_chart_data`
        // works with inclusive range. Therefore, we need to convert the range and
        // get date without time.
        let range = range.map(exclusive_datetime_range_to_inclusive);
        let (start, end) = range.map(|r| r.into_inner()).unzip();

        // At the same time, update-time relevance for local charts
        // is achieved while requesting remote source data.
        // I.e. if the range end is at some time X today,
        // the dependency will be updated with update time X,
        // so data in local DB will be relevant for time X
        // (it's reflected in `update_time` column of `charts`),
        // and `get_line_chart_data` will return the relevant data.
        // same for weeks or other resolutions.
        let start = start.map(|s| C::Resolution::from_date(s.date_naive()));
        let end = end.map(|e| C::Resolution::from_date(e.date_naive()));
        let values: Vec<TimespanValue<C::Resolution, String>> =
            get_line_chart_data::<C::Resolution>(
                cx.db,
                &C::name(),
                start,
                end,
                None,
                C::missing_date_policy(),
                false,
                C::approximate_trailing_points(),
            )
            .await?
            .into_iter()
            .map(TimespanValue::from)
            .collect();
        Ok(values)
    }
}

/// Usually the choice for line counters
pub struct DefaultQueryLast<C: ChartProperties>(PhantomData<C>);

impl<C: ChartProperties> QueryBehaviour for DefaultQueryLast<C> {
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: Option<Range<DateTimeUtc>>,
    ) -> Result<Self::Output, UpdateError> {
        let value = get_counter_data(
            cx.db,
            &C::name(),
            Some(cx.time.date_naive()),
            C::missing_date_policy(),
        )
        .await?
        .ok_or(UpdateError::Internal(format!(
            "no data for counter '{}' was found",
            C::name()
        )))?;
        Ok(value)
    }
}
