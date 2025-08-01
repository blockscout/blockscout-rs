use std::{fmt::Debug, marker::PhantomData};

use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;

use crate::{
    ChartError, ChartProperties, RequestedPointsLimit,
    charts::db_interaction::read::{get_counter_data, get_line_chart_data},
    data_source::{UpdateContext, kinds::local_db::parameter_traits::QueryBehaviour},
    range::UniversalRange,
    types::{ExtendedTimespanValue, Timespan, timespans::DateValue},
};

/// Usually the choice for line charts
pub struct DefaultQueryVec<C: ChartProperties>(PhantomData<C>);

impl<C> QueryBehaviour for DefaultQueryVec<C>
where
    C: ChartProperties,
    C::Resolution: Timespan + Ord + Debug + Clone + Send,
{
    type Output = Vec<ExtendedTimespanValue<C::Resolution, String>>;

    /// Retrieve chart data from local storage.
    ///
    /// Note that the data might have missing points for efficiency reasons.
    ///
    /// Expects metadata to be consistent with stored data
    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        points_limit: Option<RequestedPointsLimit>,
        fill_missing_dates: bool,
    ) -> Result<Self::Output, ChartError> {
        // In DB we store data with date precision. Also, `get_line_chart_data`
        // works with inclusive range. Therefore, we need to convert the range and
        // get date without time.
        let (start, end) = range.into_inclusive_pair();

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
        let values = get_line_chart_data::<C::Resolution>(
            cx.stats_db,
            &C::name(),
            start,
            end,
            points_limit,
            C::missing_date_policy(),
            fill_missing_dates,
            C::approximate_trailing_points(),
        )
        .await?;
        Ok(values)
    }
}

/// Usually the choice for line counters
pub struct DefaultQueryLast<C: ChartProperties>(PhantomData<C>);

impl<C: ChartProperties> QueryBehaviour for DefaultQueryLast<C> {
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
        _points_limit: Option<RequestedPointsLimit>,
        _fill_missing_dates: bool,
    ) -> Result<Self::Output, ChartError> {
        let value = get_counter_data(
            cx.stats_db,
            &C::name(),
            Some(cx.time.date_naive()),
            C::missing_date_policy(),
        )
        .await?
        .ok_or(ChartError::NoCounterData(C::key()))?;
        Ok(value)
    }
}

#[trait_variant::make(Send)]
pub trait ValueEstimation {
    async fn estimate(indexer: &DatabaseConnection) -> Result<DateValue<String>, ChartError>;
}

pub struct QueryLastWithEstimationFallback<E, C>(PhantomData<(E, C)>)
where
    C: ChartProperties,
    E: ValueEstimation;

impl<E, C> QueryBehaviour for QueryLastWithEstimationFallback<E, C>
where
    C: ChartProperties,
    E: ValueEstimation,
{
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
        _points_limit: Option<RequestedPointsLimit>,
        _fill_missing_dates: bool,
    ) -> Result<Self::Output, ChartError> {
        let value = match get_counter_data(
            cx.stats_db,
            &C::name(),
            Some(cx.time.date_naive()),
            C::missing_date_policy(),
        )
        .await?
        {
            Some(v) => v,
            None => E::estimate(cx.indexer_db).await?,
        };
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use chrono::NaiveDate;
    use entity::sea_orm_active_enums::ChartType;
    use pretty_assertions::assert_eq;
    use sea_orm::DatabaseConnection;

    use super::*;

    use crate::{
        ChartError, MissingDatePolicy, Named,
        data_source::{UpdateContext, UpdateParameters, types::IndexerMigrations},
        tests::init_db::init_db_all,
        types::timespans::DateValue,
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn fallback_query_works() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("fallback_query_works").await;
        let current_time = chrono::DateTime::from_str("2023-03-01T12:00:00Z").unwrap();

        let parameters = UpdateParameters::query_parameters(
            &db,
            false,
            &blockscout,
            IndexerMigrations::latest(),
            Some(current_time),
        );
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());

        struct TestFallback;

        fn expected_estimate() -> DateValue<String> {
            DateValue {
                timespan: chrono::NaiveDate::MAX,
                value: "estimate".to_string(),
            }
        }

        impl ValueEstimation for TestFallback {
            async fn estimate(
                _indexer: &DatabaseConnection,
            ) -> Result<DateValue<String>, ChartError> {
                Ok(expected_estimate())
            }
        }

        pub struct InvalidProperties;
        impl Named for InvalidProperties {
            fn name() -> String {
                "totalBlocks".into()
            }
        }
        impl ChartProperties for InvalidProperties {
            type Resolution = NaiveDate;

            fn chart_type() -> ChartType {
                ChartType::Counter
            }
            fn missing_date_policy() -> MissingDatePolicy {
                MissingDatePolicy::FillPrevious
            }
        }

        assert_eq!(
            expected_estimate(),
            QueryLastWithEstimationFallback::<TestFallback, InvalidProperties>::query_data(
                &cx,
                UniversalRange::full(),
                None,
                true
            )
            .await
            .unwrap()
        );
    }
}
