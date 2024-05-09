use std::{collections::HashSet, ops::RangeInclusive, time::Instant};

use chrono::{DateTime, Duration, NaiveDate, Utc};
use entity::charts;
use futures::{future::BoxFuture, FutureExt};
use sea_orm::{
    prelude::*, DatabaseConnection, DbErr, FromQueryResult, QuerySelect, TransactionTrait,
};

use crate::{get_chart_data, Chart, DateValue, ReadError, UpdateError};

use super::{
    create_chart,
    db_interaction::{
        chart_updaters::{
            common_operations::{
                self, get_min_block_blockscout, get_min_date_blockscout, get_nth_last_row,
            },
            ChartBatchUpdater,
        },
        write::insert_data_many,
    },
    find_chart,
};

/// Thing that can provide data from local storage.
///
/// See [`update`](`LocalDataSource::update`) and [`get_local`](`LocalDataSource::get_local`)
/// for functionality details.
///
/// Usually it's a chart that can:
///     - depend only on external data (i.e. independent from local data)
///     - depend on data from other charts
///
/// Also it can be a remote data source. In this case, `update`
pub trait DataSource {
    type PrimaryDependency: DataSource;
    type SecondaryDependencies: DataSource;
    type Output;

    /// Initialize the data source and its dependencies in local database.
    ///
    /// If the source was initialized before, keep old values.
    fn init_all_locally<'a>(
        db: &'a DatabaseConnection,
        init_time: &'a chrono::DateTime<Utc>,
    ) -> BoxFuture<'a, Result<(), DbErr>> {
        async move {
            Self::PrimaryDependency::init_all_locally(db, init_time).await?;
            Self::SecondaryDependencies::init_all_locally(db, init_time).await?;
            Self::init_itself(db, init_time).await
        }
        .boxed()
        // had to juggle with boxed futures
        // because of recursive async calls (:
        // :)
    }

    /// Initialize only this source. This fn is intended to be implemented
    /// for regular types
    fn init_itself(
        db: &DatabaseConnection,
        init_time: &chrono::DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<(), DbErr>> + Send;

    /// Update source data (values + metadata), if necessary.
    async fn update_from_remote(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(), UpdateError>;

    /// Retrieve chart data for dates in `range`.
    ///
    /// **Does not perform an update!** If you need relevant data, you likely need
    /// to call [`DataSource::update_from_remote`] beforehand.
    async fn query_data(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<Self::Output, UpdateError>;
}

// todo: instruction on how to implement
pub trait UpdateableChart: Chart {
    type PrimaryDependency: DataSource;
    type SecondaryDependencies: DataSource;

    async fn query_chart_id(db: &DatabaseConnection) -> Result<Option<i32>, UpdateError> {
        find_chart(db, Self::name())
            .await
            .map_err(UpdateError::StatsDB)
    }

    /// Create chart in db. Does not overwrite existing data.
    fn create(
        db: &DatabaseConnection,
        init_time: &chrono::DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<(), DbErr>> + Send {
        async move { create_chart(db, Self::name().into(), Self::chart_type(), init_time).await }
    }

    // todo: maybe leave only `batch_update` from fn cascade and provide helper functions to perform the batching?

    async fn batch_update(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
        update_from_row: Option<DateValue>,
        min_blockscout_block: i64,
    ) -> Result<(), UpdateError> {
        let today = cx.user_context.current_time.date_naive();
        let txn = cx
            .user_context
            .blockscout
            .begin()
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let first_date = match update_from_row {
            Some(row) => row.date,
            None => get_min_date_blockscout(&txn)
                .await
                .map(|time| time.date())
                .map_err(UpdateError::BlockscoutDB)?,
        };
        let chart_id = Self::query_chart_id(cx.user_context.db)
            .await?
            .ok_or_else(|| UpdateError::NotFound(Self::name().into()))?;

        let steps = generate_date_ranges(first_date, today, Self::step_duration());
        let n = steps.len();

        for (i, range) in steps.into_iter().enumerate() {
            tracing::info!(from =? range.start(), to =? range.end() , "run {}/{} step of batch update", i + 1, n);
            let now = Instant::now();
            let found =
                Self::update_next_values_batch(cx, chart_id, min_blockscout_block, range).await?;
            let elapsed = now.elapsed();
            tracing::info!(found =? found, elapsed =? elapsed, "{}/{} step of batch done", i + 1, n);
        }
        Self::update_metadata(cx.user_context.db, chart_id, cx.user_context.current_time).await?;
        Ok(())
    }

    /// Returns how many records were found
    async fn update_next_values_batch(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
        chart_id: i32,
        min_blockscout_block: i64,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<usize, UpdateError> {
        let primary_data = Self::PrimaryDependency::query_data(cx, range.clone()).await?;
        let secondary_data = Self::SecondaryDependencies::query_data(cx, range).await?;
        let found = Self::update_values_with(
            cx.user_context.db,
            chart_id,
            cx.user_context.current_time,
            min_blockscout_block,
            primary_data,
            secondary_data,
        )
        .await?;
        Ok(found)
    }

    /// Update chart with data from its dependencies.
    ///
    /// Returns how many records were found
    async fn update_values_with(
        db: &DatabaseConnection,
        chart_id: i32,
        update_time: chrono::DateTime<Utc>,
        min_blockscout_block: i64,
        primary_data: <Self::PrimaryDependency as DataSource>::Output,
        secondary_data: <Self::SecondaryDependencies as DataSource>::Output,
    ) -> Result<usize, UpdateError>;

    async fn update_metadata(
        db: &DatabaseConnection,
        chart_id: i32,
        update_time: chrono::DateTime<Utc>,
    ) -> Result<(), UpdateError> {
        common_operations::set_last_updated_at(chart_id, db, update_time)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(())
    }

    async fn query_data(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
        range: std::ops::RangeInclusive<sea_orm::prelude::Date>,
    ) -> Result<ChartData, UpdateError> {
        let values = get_chart_data(
            cx.user_context.db,
            Self::name(),
            Some(*range.start()),
            Some(*range.end()),
            None,
            None,
            Self::approximate_trailing_points(),
        )
        .await?
        .into_iter()
        .map(DateValue::from)
        .collect();
        let chart = charts::Entity::find()
            .column(charts::Column::Id)
            .filter(charts::Column::Name.eq(Self::name()))
            .one(cx.user_context.db)
            .await
            .map_err(ReadError::from)?
            .ok_or_else(|| ReadError::NotFound(Self::name().into()))?;
        let metadata = ChartMetadata {
            last_update: chart
                .last_updated_at
                .ok_or_else(|| ReadError::NotFound(Self::name().into()))?
                .with_timezone(&Utc),
        };
        Ok(ChartData { metadata, values })
    }
}

impl<C: UpdateableChart> DataSource for C {
    type PrimaryDependency = C::PrimaryDependency;
    type SecondaryDependencies = C::SecondaryDependencies;
    type Output = ChartData;

    async fn update_from_remote(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(), UpdateError> {
        Self::PrimaryDependency::update_from_remote(cx).await?;
        Self::SecondaryDependencies::update_from_remote(cx).await?;

        let chart_id = Self::query_chart_id(cx.user_context.db)
            .await?
            .ok_or_else(|| UpdateError::NotFound(Self::name().into()))?;
        let min_blockscout_block = get_min_block_blockscout(cx.user_context.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let offset = Some(Self::approximate_trailing_points());
        let last_updated_row = get_nth_last_row::<Self>(
            chart_id,
            min_blockscout_block,
            cx.user_context.db,
            cx.user_context.force_full,
            offset,
        )
        .await?;

        C::batch_update(cx, last_updated_row, min_blockscout_block).await?;
        Ok(())
    }

    async fn query_data(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<ChartData, UpdateError> {
        C::query_data(cx, range).await
    }

    async fn init_itself(
        db: &DatabaseConnection,
        init_time: &chrono::DateTime<Utc>,
    ) -> Result<(), DbErr> {
        Self::create(db, init_time).await
    }
}

pub fn generate_date_ranges(
    start: NaiveDate,
    end: NaiveDate,
    step: Duration,
) -> Vec<RangeInclusive<NaiveDate>> {
    let mut date_range = Vec::new();
    let mut current_date = start;

    while current_date < end {
        let next_date = current_date + step;
        date_range.push(RangeInclusive::new(current_date, next_date));
        current_date = next_date;
    }

    date_range
}

// Base case for recursive type
impl DataSource for () {
    type PrimaryDependency = ();
    type SecondaryDependencies = ();
    type Output = ();

    fn init_all_locally<'a>(
        _db: &'a DatabaseConnection,
        _init_time: &'a chrono::DateTime<Utc>,
    ) -> BoxFuture<'a, Result<(), DbErr>> {
        // stop recursion
        async { Ok(()) }.boxed()
    }

    async fn init_itself(
        _db: &DatabaseConnection,
        _init_time: &chrono::DateTime<Utc>,
    ) -> Result<(), DbErr> {
        Ok(())
    }

    async fn update_from_remote(
        _cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(), UpdateError> {
        // stop recursion
        Ok(())
    }
    async fn query_data(
        _cx: &mut UpdateContext<UpdateParameters<'_>>,
        _range: RangeInclusive<NaiveDate>,
    ) -> Result<Self::Output, UpdateError> {
        Ok(())
    }
}

impl<T1, T2> DataSource for (T1, T2)
where
    T1: DataSource,
    T2: DataSource,
{
    type PrimaryDependency = T1;
    type SecondaryDependencies = T2;
    type Output = (T1::Output, T2::Output);

    async fn init_itself(
        _db: &DatabaseConnection,
        _init_time: &chrono::DateTime<Utc>,
    ) -> Result<(), DbErr> {
        // dependencies are called in `init_all_locally`
        // the tuple itself does not need any init
        Ok(())
    }

    /// Update source data (values + metadata), if necessary.
    async fn update_from_remote(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(), UpdateError> {
        Self::PrimaryDependency::update_from_remote(cx).await?;
        Self::SecondaryDependencies::update_from_remote(cx).await?;
        Ok(())
    }

    async fn query_data(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<Self::Output, UpdateError> {
        Ok((
            T1::query_data(cx, range.clone()).await?,
            T2::query_data(cx, range).await?,
        ))
    }
}

/// Fully remote data source not controlled by this service
pub trait RemotelyPulledChart: Chart {
    type Source: ChartBatchUpdater;

    /// Returns how many records were found
    async fn update_next_values_batch(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
        chart_id: i32,
        min_blockscout_block: i64,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<usize, UpdateError> {
        let query = Self::Source::get_query(*range.start(), *range.end());
        let values = DateValue::find_by_statement(query)
            .all(cx.user_context.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let found = values.len();
        let values_model = values
            .clone()
            .into_iter()
            .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));

        insert_data_many(cx.user_context.db, values_model)
            .await
            .map_err(UpdateError::StatsDB)?;
        Ok(found)
    }

    // async fn query_local_data(
    //     cx: &mut UpdateContext<UpdateParameters<'_>>,
    //     range: std::ops::RangeInclusive<sea_orm::prelude::Date>,
    // ) -> Result<ChartData, UpdateError> {
    // }
}

impl<R: RemotelyPulledChart> UpdateableChart for R {
    type PrimaryDependency = ();
    type SecondaryDependencies = ();

    /// Returns how many records were found
    async fn update_next_values_batch(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
        chart_id: i32,
        min_blockscout_block: i64,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<usize, UpdateError> {
        <R as RemotelyPulledChart>::update_next_values_batch(
            cx,
            chart_id,
            min_blockscout_block,
            range,
        )
        .await
    }

    async fn update_values_with(
        _db: &DatabaseConnection,
        _chart_id: i32,
        _update_time: chrono::DateTime<Utc>,
        _min_blockscout_block: i64,
        _primary_data: <Self::PrimaryDependency as DataSource>::Output,
        _secondary_data: <Self::SecondaryDependencies as DataSource>::Output,
    ) -> Result<usize, UpdateError> {
        // is not called because `update_next_values_batch` is redefined
        Ok(0)
    }
}

#[derive(Clone)]
pub struct ChartMetadata {
    pub last_update: DateTime<Utc>,
}

pub struct ChartData {
    pub metadata: ChartMetadata,
    pub values: Vec<DateValue>,
}

#[derive(Clone)]
pub struct UpdateParameters<'a> {
    pub db: &'a DatabaseConnection,
    pub blockscout: &'a DatabaseConnection,
    pub current_time: chrono::DateTime<Utc>,
    pub force_full: bool,
}

pub struct UpdateContext<UCX> {
    // todo: consider memoization
    // update_results: HashMap<String, (Vec<DateValue>, ChartMetadata)>,
    pub user_context: UCX,
}

impl<'a, UCX> UpdateContext<UCX> {
    pub fn from_inner(inner: UCX) -> Self {
        Self {
            // update_results: HashMap::new(),
            user_context: inner,
        }
    }
}

// todo: move comments somewhere
/// Directed Acyclic Connected Graph
pub trait UpdateGroup<P> {
    // todo: impl with macros(?)
    async fn create_charts(
        db: &DatabaseConnection,
        enabled_names: &HashSet<String>,

        current_time: &chrono::DateTime<Utc>,
    ) -> Result<(), DbErr>;
    async fn update_charts(params: P, enabled_names: &HashSet<String>) -> Result<(), UpdateError>;
}

// mod update_strategies {
//     fn
// }

#[cfg(test)]
mod examples {
    use std::{collections::HashSet, str::FromStr};

    use chrono::{DateTime, Utc};
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::prelude::*;

    use super::{
        DataSource, RemotelyPulledChart, UpdateContext, UpdateGroup, UpdateParameters,
        UpdateableChart,
    };
    use crate::{
        charts::db_interaction::{chart_updaters::parse_and_cumsum, write::insert_data_many},
        lines::NewContracts,
        tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
        Chart, MissingDatePolicy, UpdateError,
    };

    struct NewContractsChart;

    impl crate::Chart for NewContractsChart {
        fn name() -> &'static str {
            "newContracts"
        }

        fn chart_type() -> ChartType {
            ChartType::Line
        }
    }

    impl RemotelyPulledChart for NewContractsChart {
        type Source = NewContracts;
    }

    struct ContractsGrowthChart;

    impl Chart for ContractsGrowthChart {
        fn name() -> &'static str {
            "contractsGrowth"
        }
        fn chart_type() -> ChartType {
            ChartType::Line
        }
        fn missing_date_policy() -> MissingDatePolicy {
            MissingDatePolicy::FillPrevious
        }
    }

    impl UpdateableChart for ContractsGrowthChart {
        type PrimaryDependency = NewContractsChart;
        type SecondaryDependencies = ();

        async fn update_values_with(
            db: &DatabaseConnection,
            chart_id: i32,
            _update_time: chrono::DateTime<Utc>,
            min_blockscout_block: i64,
            primary_data: <Self::PrimaryDependency as DataSource>::Output,
            _secondary_data: <Self::SecondaryDependencies as DataSource>::Output,
        ) -> Result<usize, UpdateError> {
            let found = primary_data.values.len();
            let values =
                parse_and_cumsum::<i64>(primary_data.values, Self::PrimaryDependency::name())?
                    .into_iter()
                    .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));
            insert_data_many(db, values)
                .await
                .map_err(UpdateError::StatsDB)?;
            Ok(found)
        }
    }

    pub struct ExampleUpdateGroup;

    impl<'a> UpdateGroup<UpdateParameters<'a>> for ExampleUpdateGroup {
        async fn create_charts(
            db: &DatabaseConnection,
            enabled_names: &HashSet<String>,
            current_time: &chrono::DateTime<Utc>,
        ) -> Result<(), DbErr> {
            if enabled_names.contains(NewContractsChart::name()) {
                NewContractsChart::init_all_locally(db, current_time).await?;
            }
            if enabled_names.contains(ContractsGrowthChart::name()) {
                ContractsGrowthChart::init_all_locally(db, current_time).await?;
            }
            Ok(())
        }

        async fn update_charts(
            params: UpdateParameters<'a>,
            enabled_names: &HashSet<String>,
        ) -> Result<(), UpdateError> {
            let mut cx = UpdateContext::from_inner(params.into());
            if enabled_names.contains(NewContractsChart::name()) {
                NewContractsChart::update_from_remote(&mut cx).await?;
            }
            if enabled_names.contains(ContractsGrowthChart::name()) {
                ContractsGrowthChart::update_from_remote(&mut cx).await?;
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn _update_examples() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_examples").await;
        let current_time = DateTime::from_str("2023-03-01T12:00:00Z").unwrap();
        let current_date = current_time.date_naive();
        fill_mock_blockscout_data(&blockscout, current_date).await;
        let enabled = HashSet::from(
            [NewContractsChart::name(), ContractsGrowthChart::name()].map(|l| l.to_owned()),
        );
        ExampleUpdateGroup::create_charts(&db, &enabled, &current_time)
            .await
            .unwrap();

        let parameters = UpdateParameters {
            db: &db,
            blockscout: &blockscout,
            current_time,
            force_full: true,
        };
        ExampleUpdateGroup::update_charts(parameters, &enabled)
            .await
            .unwrap();
    }
}
