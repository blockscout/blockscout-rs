use std::{collections::HashSet, marker::PhantomData, ops::RangeInclusive, time::Instant};

use chrono::{DateTime, Duration, NaiveDate, Utc};
use sea_orm::{prelude::*, DatabaseConnection, DbErr, FromQueryResult, TransactionTrait};

use crate::{Chart, ChartUpdater, DateValue, UpdateError};

use super::{
    db_interaction::chart_updaters::{
        common_operations::{get_min_block_blockscout, get_min_date_blockscout, get_nth_last_row},
        ChartBatchUpdater, ChartPartialUpdater,
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

        let steps = generate_date_ranges(first_date, today, Self::step_duration());
        let n = steps.len();

        for (i, range) in steps.into_iter().enumerate() {
            tracing::info!(from =? range.start(), to =? range.end() , "run {}/{} step of batch update", i + 1, n);
            let now = Instant::now();
            let found = Self::update_next_batch(cx, min_blockscout_block, range).await?;
            let elapsed = now.elapsed();
            tracing::info!(found =? found, elapsed =? elapsed, "{}/{} step of batch done", i + 1, n);
        }
        Ok(())
    }

    /// Returns how many records were found
    async fn update_next_batch(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
        min_blockscout_block: i64,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<usize, UpdateError> {
        let primary_data = Self::PrimaryDependency::query_data(cx, range.clone()).await?;
        let secondary_data = Self::SecondaryDependencies::query_data(cx, range).await?;
        let found = Self::update_with(
            cx.user_context.db,
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
    async fn update_with(
        db: &DatabaseConnection,
        update_time: chrono::DateTime<Utc>,
        min_blockscout_block: i64,
        primary_data: <Self::PrimaryDependency as DataSource>::Output,
        secondary_data: <Self::SecondaryDependencies as DataSource>::Output,
    ) -> Result<usize, UpdateError>;

    ///
    async fn query_data(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
        range: std::ops::RangeInclusive<sea_orm::prelude::Date>,
    ) -> Result<ChartData, UpdateError>;
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

impl DataSource for () {
    type PrimaryDependency = ();
    type SecondaryDependencies = ();
    type Output = ();

    async fn update_from_remote(
        _cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(), UpdateError> {
        // Recursion termination
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
pub trait RemotePull {
    async fn get_values(
        blockscout: &DatabaseConnection,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<Vec<DateValue>, UpdateError>;
}

// todo: move to examples/remove
struct RemoteSource<Inner: ChartUpdater> {
    _inner: PhantomData<Inner>,
}

impl<Inner: ChartPartialUpdater> RemotePull for RemoteSource<Inner> {
    async fn get_values(
        blockscout: &DatabaseConnection,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        Inner::get_values(
            blockscout,
            Some(DateValue {
                date: *range.start(),
                value: "".to_owned(),
            }),
        )
        .await
    }
}

// todo: remove adapter and impl natively
impl<T: ChartBatchUpdater> RemotePull for T {
    async fn get_values(
        blockscout: &DatabaseConnection,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let query = Self::get_query(*range.start(), *range.end());
        let values = DateValue::find_by_statement(query)
            .all(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        Ok(values)
    }
}

macro_rules! impl_data_source_for_remote_pull_type {
    ($type:ty) => {
        impl DataSource for $type {
            type PrimaryDependency = ();
            type SecondaryDependencies = ();
            type Output = Vec<crate::DateValue>;

            /// Does not do anything because source is not controlled by this service.
            async fn update_from_remote(
                _cx: &mut crate::charts::data_source::UpdateContext<
                    crate::charts::data_source::UpdateParameters<'_>,
                >,
            ) -> Result<(), crate::UpdateError> {
                Ok(())
            }

            /// Retrieve data for dates in `range`.
            async fn query_data(
                cx: &mut crate::charts::data_source::UpdateContext<
                    crate::charts::data_source::UpdateParameters<'_>,
                >,
                range: crate::charts::data_source::RangeInclusive<chrono::NaiveDate>,
            ) -> Result<Self::Output, crate::UpdateError> {
                <Self as crate::charts::data_source::RemotePull>::get_values(
                    cx.user_context.blockscout,
                    range,
                )
                .await
            }
        }
    };
}

#[derive(Clone)]
pub struct ChartMetadata {
    pub last_update: DateTime<Utc>,
}

pub struct ChartData {
    pub metadata: ChartMetadata,
    pub values: Vec<DateValue>,
}

// todo: add
// pub struct BatchRange {}

// todo: rename
// pub trait SuperCoolChartUpdater: Chart + DataSource {}

// // todo: rename
// pub trait CoolChartUpdater: Chart {
//     type PrimaryDependency: DataSource;
//     type SecondaryDependencies: OtherCharts;

//     /// Update only data (values) of the chart (`chart_data` table).
//     ///
//     /// Return the update result.
//     ///
//     /// Implementation is expected to be highly variable.
//     async fn update_values(
//         cx: &mut UpdateContext<UpdateParameters<'_>>,
//     ) -> Result<(), UpdateError> {
//         // todo: check last_updated_at (with `force_full`) and update if necessary

//         // todo: batching
//         let data = Self::PrimaryDependency::retrieve_data(cx).await?;
//         let _other_data = Self::SecondaryDependencies::retrieve_data(cx).await?;
//         // todo: update ??? save??
//         Ok(data.0)
//     }

//     // async fn update_values_inner()

//     /// Update only metadata of the chart (`charts` table).
//     ///
//     /// Return the update result.
//     ///
//     /// Generally better to call after changing chart data to keep
//     /// the info relevant (i.e. if it depends on values).
//     async fn update_metadata(
//         cx: &mut UpdateContext<UpdateParameters<'_>>,
//     ) -> Result<ChartMetadata, UpdateError> {
//         let chart_id = find_chart(cx.user_context.db, Self::name())
//             .await
//             .map_err(UpdateError::StatsDB)?
//             .ok_or_else(|| UpdateError::NotFound(Self::name().into()))?;
//         common_operations::set_last_updated_at(
//             chart_id,
//             cx.user_context.db,
//             cx.user_context.current_time,
//         )
//         .await
//         .map_err(UpdateError::StatsDB)?;
//         Ok(ChartMetadata {
//             last_update: cx.user_context.current_time,
//         })
//     }

//     /// Update data and metadata of the chart. Return the results.
//     ///
//     /// `current_time` is settable mainly for testing purposes. So that
//     /// code dependant on time (mostly metadata updates) can be reproducibly tested.
//     async fn update_data(cx: &mut UpdateContext<UpdateParameters<'_>>) -> Result<(), UpdateError> {
//         Self::update_values(cx).await?;
//         Self::update_metadata(cx).await?;
//         Ok(())
//     }

//     /// Retrieve chart data from local storage `db` for dates in `range`.
//     ///
//     /// **Does not perform an update!** If you need relevant data, call
//     /// [`Self::update_data`] beforehand.
//     async fn get_data(
//         db: &DatabaseConnection,
//         range: RangeInclusive<NaiveDate>,
//     ) -> Result<(Vec<DateValue>, ChartMetadata), UpdateError>;

//     /// Run [`Self::update`] with acquiring global mutex for the chart
//     async fn update_with_mutex(
//         cx: &mut UpdateContext<UpdateParameters<'_>>,
//     ) -> Result<(), UpdateError> {
//         // todo: verify that mutex is not needed
//         // let name = Self::name();
//         // let mutex = get_global_update_mutex(name).await;
//         // let _permit = {
//         //     match mutex.try_lock() {
//         //         Ok(v) => v,
//         //         Err(_) => {
//         //             tracing::warn!(
//         //                 chart_name = name,
//         //                 "found locked update mutex, waiting for unlock"
//         //             );
//         //             mutex.lock().await
//         //         }
//         //     }
//         // };
//         Self::update_data(cx).await
//     }
// }

#[derive(Clone)]
pub struct UpdateParameters<'a> {
    pub db: &'a DatabaseConnection,
    pub blockscout: &'a DatabaseConnection,
    pub current_time: chrono::DateTime<Utc>,
    pub force_full: bool,
}

pub struct UpdateContext<UCX> {
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
    ) -> Result<(), DbErr>;
    async fn update_charts(params: P, enabled_names: &HashSet<String>) -> Result<(), UpdateError>;
}

#[cfg(test)]
mod examples {
    use std::{collections::HashSet, str::FromStr};

    use chrono::{DateTime, Utc};
    use entity::{charts, sea_orm_active_enums::ChartType};
    use sea_orm::{prelude::*, QuerySelect};

    use super::{
        ChartData, ChartMetadata, DataSource, UpdateContext, UpdateGroup, UpdateParameters,
        UpdateableChart,
    };
    use crate::{
        charts::{
            create_chart,
            db_interaction::{
                chart_updaters::{common_operations, parse_and_cumsum},
                write::insert_data_many,
            },
        },
        get_chart_data,
        lines::NewContracts,
        tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
        Chart, DateValue, MissingDatePolicy, ReadError, UpdateError,
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

    impl_data_source_for_remote_pull_type!(NewContracts);

    impl UpdateableChart for NewContractsChart {
        type PrimaryDependency = NewContracts;
        type SecondaryDependencies = ();

        async fn update_with(
            db: &DatabaseConnection,
            update_time: chrono::DateTime<Utc>,
            min_blockscout_block: i64,
            primary_data: <Self::PrimaryDependency as DataSource>::Output,
            _secondary_data: <Self::SecondaryDependencies as DataSource>::Output,
        ) -> Result<usize, UpdateError> {
            // todo: automatic impl??

            let chart_id = Self::query_chart_id(db)
                .await?
                .ok_or_else(|| UpdateError::NotFound(Self::name().into()))?;
            let found = primary_data.len();
            let values = primary_data
                .clone()
                .into_iter()
                .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));

            insert_data_many(db, values)
                .await
                .map_err(UpdateError::StatsDB)?;
            common_operations::set_last_updated_at(chart_id, db, update_time)
                .await
                .map_err(UpdateError::StatsDB)?;
            Ok(found)
        }

        async fn query_data(
            cx: &mut UpdateContext<UpdateParameters<'_>>,
            range: std::ops::RangeInclusive<sea_orm::prelude::Date>,
        ) -> Result<ChartData, UpdateError> {
            let values = get_chart_data(
                cx.user_context.db,
                Self::PrimaryDependency::name(),
                Some(*range.start()),
                Some(*range.end()),
                None,
                None,
                Self::PrimaryDependency::approximate_trailing_points(),
            )
            .await?
            .into_iter()
            .map(DateValue::from)
            .collect();
            let chart = charts::Entity::find()
                .column(charts::Column::Id)
                .filter(charts::Column::Name.eq(Self::PrimaryDependency::name()))
                .one(cx.user_context.db)
                .await
                .map_err(ReadError::from)?
                .ok_or_else(|| ReadError::NotFound(Self::PrimaryDependency::name().into()))?;
            let metadata = ChartMetadata {
                last_update: chart
                    .last_updated_at
                    .ok_or_else(|| ReadError::NotFound(Self::PrimaryDependency::name().into()))?
                    .with_timezone(&Utc),
            };
            Ok(ChartData { metadata, values })
        }
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

        async fn create(db: &DatabaseConnection) -> Result<(), DbErr> {
            NewContracts::create(db).await?;
            create_chart(db, Self::name().into(), Self::chart_type()).await
        }
    }

    impl UpdateableChart for ContractsGrowthChart {
        type PrimaryDependency = NewContractsChart;
        type SecondaryDependencies = ();

        async fn update_with(
            db: &DatabaseConnection,
            update_time: chrono::DateTime<Utc>,
            min_blockscout_block: i64,
            primary_data: <Self::PrimaryDependency as DataSource>::Output,
            _secondary_data: <Self::SecondaryDependencies as DataSource>::Output,
        ) -> Result<usize, UpdateError> {
            let chart_id = Self::query_chart_id(db)
                .await?
                .ok_or_else(|| UpdateError::NotFound(Self::name().into()))?;
            let found = primary_data.values.len();
            let values = parse_and_cumsum::<i64>(primary_data.values, NewContracts::name())?
                .into_iter()
                .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));
            insert_data_many(db, values)
                .await
                .map_err(UpdateError::StatsDB)?;
            common_operations::set_last_updated_at(chart_id, db, update_time)
                .await
                .map_err(UpdateError::StatsDB)?;
            Ok(found)
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
                .filter(charts::Column::Name.eq(Self::PrimaryDependency::name()))
                .one(cx.user_context.db)
                .await
                .map_err(ReadError::from)?
                .ok_or_else(|| ReadError::NotFound(Self::PrimaryDependency::name().into()))?;
            let metadata = ChartMetadata {
                last_update: chart
                    .last_updated_at
                    .ok_or_else(|| ReadError::NotFound(Self::PrimaryDependency::name().into()))?
                    .with_timezone(&Utc),
            };
            Ok(ChartData { metadata, values })
        }
    }

    pub struct ExampleUpdateGroup;

    impl<'a> UpdateGroup<UpdateParameters<'a>> for ExampleUpdateGroup {
        async fn create_charts(
            db: &DatabaseConnection,
            enabled_names: &HashSet<String>,
        ) -> Result<(), DbErr> {
            if enabled_names.contains(NewContractsChart::name()) {
                NewContractsChart::create(db).await?;
            }
            if enabled_names.contains(ContractsGrowthChart::name()) {
                ContractsGrowthChart::create(db).await?;
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
        ExampleUpdateGroup::create_charts(&db, &enabled)
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
