use std::{collections::HashSet, marker::PhantomData, ops::RangeInclusive};

use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::{DatabaseConnection, FromQueryResult};

use crate::{ChartUpdater, DateValue, UpdateError};

use super::db_interaction::chart_updaters::{ChartBatchUpdater, ChartPartialUpdater};

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
    ) -> Result<(), UpdateError> {
        Self::PrimaryDependency::update_from_remote(cx).await?;
        Self::SecondaryDependencies::update_from_remote(cx).await?;
        // todo: batching and different dates
        let primary_data = Self::PrimaryDependency::query_data(
            cx,
            RangeInclusive::new(
                NaiveDate::from_ymd_opt(2015, 01, 01).unwrap(),
                NaiveDate::from_ymd_opt(2025, 01, 01).unwrap(),
            ),
        )
        .await?;
        let secondary_data = Self::SecondaryDependencies::query_data(
            cx,
            RangeInclusive::new(
                NaiveDate::from_ymd_opt(2015, 01, 01).unwrap(),
                NaiveDate::from_ymd_opt(2025, 01, 01).unwrap(),
            ),
        )
        .await?;
        Self::update_with(cx.user_context.db, primary_data, secondary_data).await?;
        Ok(())
    }

    async fn update_with(
        db: &DatabaseConnection,
        primary_data: <Self::PrimaryDependency as DataSource>::Output,
        secondary_data: <Self::SecondaryDependencies as DataSource>::Output,
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

    async fn update_with(
        _db: &DatabaseConnection,
        _primary_data: (),
        _secondary_data: <Self::SecondaryDependencies as DataSource>::Output,
    ) -> Result<(), UpdateError> {
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

    async fn update_with(
        _db: &DatabaseConnection,
        _primary_data: <Self::PrimaryDependency as DataSource>::Output,
        _secondary_data: <Self::SecondaryDependencies as DataSource>::Output,
    ) -> Result<(), UpdateError> {
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

pub trait RemotePull {
    async fn get_values(
        blockscout: &DatabaseConnection,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<Vec<DateValue>, UpdateError>;
}

// todo: move to examples/remove
#[allow(unused)]
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

impl<T: RemotePull> DataSource for T {
    // async fn retrieve_data(
    //     cx: &mut UpdateContext<UpdateParameters<'_>>,
    // ) -> Result<(Vec<DateValue>, ChartMetadata), UpdateError> {
    //     if let Some(data) = cx.update_results.get(Inner::name()) {
    //         return Ok(data.clone());
    //     }
    //     Inner::update_with_mutex(cx).await
    // }

    type PrimaryDependency = ();
    type SecondaryDependencies = ();
    type Output = Vec<DateValue>;

    async fn update_with(
        _db: &DatabaseConnection,
        _primary_data: (),
        _secondary_data: (),
    ) -> Result<(), UpdateError> {
        Ok(())
    }

    async fn query_data(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
        range: RangeInclusive<NaiveDate>,
    ) -> Result<Self::Output, UpdateError> {
        Self::get_values(cx.user_context.blockscout, range).await
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
    async fn update_charts(params: P, enabled_names: HashSet<String>) -> Result<(), UpdateError>;
}

#[cfg(test)]
mod examples {
    use std::{collections::HashSet, str::FromStr};

    use chrono::{DateTime, Utc};
    use entity::{charts, sea_orm_active_enums::ChartType};
    use sea_orm::{prelude::*, QuerySelect};

    use super::{
        ChartData, ChartMetadata, DataSource, UpdateContext, UpdateGroup, UpdateParameters,
    };
    use crate::{
        charts::{
            create_chart,
            db_interaction::{chart_updaters::parse_and_cumsum, write::insert_data_many},
            find_chart,
        },
        get_chart_data,
        lines::NewContracts,
        tests::init_db::init_db_all,
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

    impl DataSource for NewContractsChart {
        type PrimaryDependency = NewContracts;
        type SecondaryDependencies = ();
        type Output = ChartData;

        async fn update_with(
            db: &sea_orm::prelude::DatabaseConnection,
            primary_data: <Self::PrimaryDependency as DataSource>::Output,
            _secondary_data: <Self::SecondaryDependencies as DataSource>::Output,
        ) -> Result<(), UpdateError> {
            // todo: automatic impl??

            let chart_id = find_chart(db, Self::name())
                .await
                .map_err(UpdateError::StatsDB)?
                .ok_or_else(|| UpdateError::NotFound(Self::name().into()))?;
            let values = primary_data
                .clone()
                .into_iter()
                .map(|value| value.active_model(chart_id, None));
            insert_data_many(db, values)
                .await
                .map_err(UpdateError::StatsDB)?;
            Ok(())
        }

        async fn query_data(
            cx: &mut UpdateContext<UpdateParameters<'_>>,
            range: std::ops::RangeInclusive<sea_orm::prelude::Date>,
        ) -> Result<Self::Output, UpdateError> {
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

    // impl RemotePull for NewContractsChart {
    //     async fn get_values(
    //         blockscout: &sea_orm::prelude::DatabaseConnection,
    //         range: std::ops::RangeInclusive<sea_orm::prelude::Date>,
    //     ) -> Result<Vec<DateValue>, UpdateError> {
    //         NewContracts::get_values(blockscout, range).await
    //     }
    // }

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

    impl DataSource for ContractsGrowthChart {
        type PrimaryDependency = NewContractsChart;
        type SecondaryDependencies = ();
        type Output = ChartData;

        async fn update_with(
            db: &sea_orm::prelude::DatabaseConnection,
            primary_data: <Self::PrimaryDependency as DataSource>::Output,
            _secondary_data: <Self::SecondaryDependencies as DataSource>::Output,
        ) -> Result<(), UpdateError> {
            let chart_id = find_chart(db, Self::name())
                .await
                .map_err(UpdateError::StatsDB)?
                .ok_or_else(|| UpdateError::NotFound(Self::name().into()))?;
            let values = parse_and_cumsum::<i64>(primary_data.values, NewContracts::name())?
                .into_iter()
                .map(|value| value.active_model(chart_id, None));
            insert_data_many(db, values)
                .await
                .map_err(UpdateError::StatsDB)?;
            Ok(())
        }

        async fn query_data(
            cx: &mut UpdateContext<UpdateParameters<'_>>,
            range: std::ops::RangeInclusive<sea_orm::prelude::Date>,
        ) -> Result<Self::Output, UpdateError> {
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

    #[allow(unused)]
    pub struct ExampleUpdateGroup;

    impl<'a> UpdateGroup<UpdateParameters<'a>> for ExampleUpdateGroup {
        async fn update_charts(
            params: UpdateParameters<'a>,
            enabled_names: HashSet<String>,
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
        let (db, blockscout) = init_db_all("update_examples").await;
        let current_time = DateTime::from_str("2023-03-01T12:00:00Z").unwrap();

        let parameters = UpdateParameters {
            db: &db,
            blockscout: &blockscout,
            current_time,
            force_full: true,
        };
        ExampleUpdateGroup::update_charts(
            parameters,
            HashSet::from(
                [NewContractsChart::name(), ContractsGrowthChart::name()].map(|l| l.to_owned()),
            ),
        )
        .await
        .unwrap();
    }
}
