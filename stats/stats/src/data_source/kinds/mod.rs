//! To simplify implementation of overly-generic `DataSource` trait
//! as well as to reduce boilerblate, types in this module can
//! be used.
//!
//! Generally, they are represented as types (or type aliases)
//! with generic parameters = parameters for the particular kind.
//!
//! [More details on data sources](crate::data_source)
//!
//! ## Dependency requirements
//! Some data sources have additional constraints on data sources
//! that are not checked in code. The dependency(-ies) might return
//! data with some dates missing. Usually this means either
//! - Value for the date == value for the previous date (==[`MissingDatePolicy::FillPrevious`](crate::charts::MissingDatePolicy))
//! - Value is 0 (==[`MissingDatePolicy::FillZero`](crate::charts::MissingDatePolicy))
//!
//! It's not checked in the code since it is both expected to work
//! for charts with set `MissingDatePolicy` and other sources
//! without such parameters.
//!
//! Implementing this seems like a high-effort job + these errors
//! should be caught by tests very fast (or be obvious in runtime),
//! so it's left as this notice. It should be linked to the data
//! sources' docs where applicable.

use std::future::Future;

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};

use crate::{range::UniversalRange, ChartError, ChartKey};

use super::{DataSource, UpdateContext};

pub mod auxiliary;
pub mod data_manipulation;
pub mod local_db;
pub mod remote_db;

/// Any data source that just passes data and maybe does some transformations
/// on it in the process. Does not participate in updates and initialization.
pub trait AdapterDataSource {
    /// See [`DataSource::MainDependencies`]
    type MainDependencies: DataSource;
    /// See [`DataSource::ResolutionDependencies`]
    type ResolutionDependencies: DataSource;
    /// See [`DataSource::Output`]
    type Output: Send;

    /// See [`DataSource::query_data`]
    fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> impl Future<Output = Result<Self::Output, ChartError>> + Send;
}

impl<A: AdapterDataSource> DataSource for A {
    type MainDependencies = <A as AdapterDataSource>::MainDependencies;
    type ResolutionDependencies = <A as AdapterDataSource>::ResolutionDependencies;
    type Output = <A as AdapterDataSource>::Output;

    fn chart_key() -> Option<ChartKey> {
        None
    }

    fn mutex_id() -> Option<String> {
        None
    }

    async fn init_itself(
        _db: &DatabaseConnection,
        _init_time: &DateTime<Utc>,
    ) -> Result<(), DbErr> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), ChartError> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn set_next_update_from_itself(
        _db: &DatabaseConnection,
        _update_from: chrono::NaiveDate,
    ) -> Result<(), ChartError> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, ChartError> {
        <A as AdapterDataSource>::query_data(cx, range, dependency_data_fetch_timer).await
    }
}
