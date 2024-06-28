use std::{marker::PhantomData, ops::RangeInclusive};

use sea_orm::prelude::DateTimeUtc;

use crate::{
    charts::db_interaction::read::get_counter_data,
    data_source::{kinds::local_db::parameter_traits::QueryBehaviour, UpdateContext},
    get_line_chart_data, ChartProperties, DateValueString, UpdateError,
};

/// Usually the choice for line charts
pub struct DefaultQueryVec<C: ChartProperties>(PhantomData<C>);

impl<C: ChartProperties> QueryBehaviour for DefaultQueryVec<C> {
    type Output = Vec<DateValueString>;

    /// Retrieve chart data from local storage.
    ///
    /// Note that the data might have missing points for efficiency reasons.
    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<RangeInclusive<DateTimeUtc>>,
    ) -> Result<Self::Output, UpdateError> {
        let (start, end) = range.map(|r| (*r.start(), *r.end())).unzip();
        // Currently we store data with date precision
        let start = start.map(|s| s.date_naive());
        let end = end.map(|s| s.date_naive());
        let values: Vec<DateValueString> = get_line_chart_data(
            cx.db,
            C::NAME,
            start,
            end,
            None,
            C::missing_date_policy(),
            false,
            C::approximate_trailing_points(),
        )
        .await?
        .into_iter()
        .map(DateValueString::from)
        .collect();
        Ok(values)
    }
}

/// Usually the choice for line counters
pub struct DefaultQueryLast<C: ChartProperties>(PhantomData<C>);

impl<C: ChartProperties> QueryBehaviour for DefaultQueryLast<C> {
    type Output = DateValueString;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: Option<RangeInclusive<DateTimeUtc>>,
    ) -> Result<Self::Output, UpdateError> {
        let value = get_counter_data(
            cx.db,
            C::NAME,
            Some(cx.time.date_naive()),
            C::missing_date_policy(),
        )
        .await?
        .ok_or(UpdateError::Internal(format!(
            "no data for counter '{}' was found",
            C::NAME
        )))?;
        Ok(value)
    }
}
