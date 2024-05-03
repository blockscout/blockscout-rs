use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
};

use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;

use crate::{Chart, ChartUpdater, DateValue, UpdateError};

use super::{db_interaction::chart_updaters::common_operations, find_chart};

pub trait DataSource {
    async fn retrieve_data(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(Vec<DateValue>, ChartMetadata), UpdateError>;
}

struct SqlDataSource<Inner: ChartUpdater> {
    _inner: PhantomData<Inner>,
}

impl<Inner: ChartUpdater> DataSource for SqlDataSource<Inner> {
    async fn retrieve_data(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(Vec<DateValue>, ChartMetadata), UpdateError> {
        if let Some(data) = cx.update_results.get(Inner::name()) {
            return Ok(data.clone());
        }
        Inner::update_with_mutex(cx).await
    }
}

impl<C: CoolChartUpdater> DataSource for C {
    async fn retrieve_data(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(Vec<DateValue>, ChartMetadata), UpdateError> {
        Self::update_return(cx).await
    }
}

pub trait OtherCharts {
    type Output;
    async fn retrieve_data(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<Self::Output, UpdateError>;
}

impl OtherCharts for () {
    type Output = ();

    async fn retrieve_data(
        _cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<Self::Output, UpdateError> {
        Ok(())
    }
}

impl<C: CoolChartUpdater> OtherCharts for C {
    type Output = (Vec<DateValue>, ChartMetadata);

    async fn retrieve_data(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<Self::Output, UpdateError> {
        C::update_return(cx).await
    }
}

impl<C1: CoolChartUpdater, C2: CoolChartUpdater> OtherCharts for (C1, C2) {
    type Output = (
        (Vec<DateValue>, ChartMetadata),
        (Vec<DateValue>, ChartMetadata),
    );

    async fn retrieve_data(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<Self::Output, UpdateError> {
        Ok((C1::update_return(cx).await?, C2::update_return(cx).await?))
    }
}

#[derive(Clone)]
pub struct ChartMetadata {
    pub last_update: DateTime<Utc>,
}

// todo: add
// pub struct BatchRange {}

// todo: rename
pub trait CoolChartUpdater: Chart {
    type DS: DataSource;
    type OC: OtherCharts;

    /// Update only data (values) of the chart (`chart_data` table).
    ///
    /// Return the update result.
    ///
    /// Implementation is expected to be highly variable.
    async fn update_values(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        // todo: update memoization in context
        let data = Self::DS::retrieve_data(cx).await?;
        let _other_data = Self::OC::retrieve_data(cx).await?;
        // todo: update ??? save??
        Ok(data.0)
    }

    /// Update only metadata of the chart (`charts` table).
    ///
    /// Return the update result.
    ///
    /// Generally better to call after changing chart data to keep
    /// the info relevant (i.e. if it depends on values).
    async fn update_metadata(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<ChartMetadata, UpdateError> {
        let chart_id = find_chart(cx.user_context.db, Self::name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(Self::name().into()))?;
        common_operations::set_last_updated_at(
            chart_id,
            cx.user_context.db,
            cx.user_context.current_time,
        )
        .await
        .map_err(UpdateError::StatsDB)?;
        Ok(ChartMetadata {
            last_update: cx.user_context.current_time,
        })
    }

    /// Update data and metadata of the chart. Return the results.
    ///
    /// `current_time` is settable mainly for testing purposes. So that
    /// code dependant on time (mostly metadata updates) can be reproducibly tested.
    async fn update_return(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(Vec<DateValue>, ChartMetadata), UpdateError> {
        Ok((
            Self::update_values(cx).await?,
            Self::update_metadata(cx).await?,
        ))
    }

    /// Run [`Self::update`] with acquiring global mutex for the chart
    async fn update_with_mutex(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<(Vec<DateValue>, ChartMetadata), UpdateError> {
        // todo: verify that mutex is not needed
        // let name = Self::name();
        // let mutex = get_global_update_mutex(name).await;
        // let _permit = {
        //     match mutex.try_lock() {
        //         Ok(v) => v,
        //         Err(_) => {
        //             tracing::warn!(
        //                 chart_name = name,
        //                 "found locked update mutex, waiting for unlock"
        //             );
        //             mutex.lock().await
        //         }
        //     }
        // };
        Self::update_return(cx).await
    }
}

#[derive(Clone)]
pub struct UpdateParameters<'a> {
    pub db: &'a DatabaseConnection,
    pub blockscout: &'a DatabaseConnection,
    pub current_time: chrono::DateTime<Utc>,
    pub force_full: bool,
}

pub struct UpdateContext<UCX> {
    update_results: HashMap<String, (Vec<DateValue>, ChartMetadata)>,
    pub user_context: UCX,
}

impl<'a, UCX> UpdateContext<UCX> {
    pub fn from_inner(inner: UCX) -> Self {
        Self {
            update_results: HashMap::new(),
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

    use chrono::DateTime;
    use entity::sea_orm_active_enums::ChartType;

    use super::{CoolChartUpdater, SqlDataSource, UpdateContext, UpdateGroup, UpdateParameters};
    use crate::{lines::NewContracts, tests::init_db::init_db_all, Chart, UpdateError};

    struct NewContractsChart;

    impl crate::Chart for NewContractsChart {
        fn name() -> &'static str {
            "newContracts"
        }

        fn chart_type() -> ChartType {
            ChartType::Line
        }
    }

    impl CoolChartUpdater for NewContractsChart {
        type DS = SqlDataSource<NewContracts>;
        type OC = ();
    }

    struct ContractsGrowthChart;

    impl Chart for ContractsGrowthChart {
        fn name() -> &'static str {
            "contractsGrowth"
        }

        fn chart_type() -> ChartType {
            ChartType::Line
        }
    }

    impl CoolChartUpdater for ContractsGrowthChart {
        type DS = NewContractsChart;
        type OC = ();
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
                NewContractsChart::update_return(&mut cx).await?;
            }
            if enabled_names.contains(ContractsGrowthChart::name()) {
                ContractsGrowthChart::update_return(&mut cx).await?;
            }
            Ok(())
        }
    }

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
