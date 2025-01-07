//! There is no unified trait for "chart". However, within the
//! scope of this project, "chart" can be thought of as
//! something that implements [`DataSource`](crate::data_source::DataSource),
//! [`trait@ChartProperties`], and is stored in local database (e.g.
//! [`LocalDbChartSource`](crate::data_source::kinds::local_db))

use std::fmt::Display;

use crate::{types::Timespan, ReadError};
use chrono::{DateTime, Utc};
use entity::sea_orm_active_enums::{ChartResolution, ChartType};
use sea_orm::prelude::*;
use thiserror::Error;

use super::{
    db_interaction::read::ApproxUnsignedDiff,
    query_dispatch::{ChartTypeSpecifics, QuerySerialized, QuerySerializedDyn},
};

#[derive(Error, Debug)]
pub enum ChartError {
    #[error("blockscout database error: {0}")]
    BlockscoutDB(DbErr),
    #[error("stats database error: {0}")]
    StatsDB(DbErr),
    #[error("chart {0} not found")]
    ChartNotFound(ChartKey),
    #[error("exceeded limit on requested data points (~{limit}); choose smaller time interval.")]
    IntervalTooLarge { limit: u32 },
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<ReadError> for ChartError {
    fn from(read: ReadError) -> Self {
        match read {
            ReadError::DB(db) => ChartError::StatsDB(db),
            ReadError::ChartNotFound(err) => ChartError::ChartNotFound(err),
            ReadError::IntervalTooLarge(limit) => ChartError::IntervalTooLarge { limit },
        }
    }
}

#[derive(Clone)]
pub struct ChartMetadata {
    pub id: i32,
    #[allow(unused)]
    pub created_at: DateTime<Utc>,
    /// NOTE: this timestamp is truncated when inserted
    /// into a database
    pub last_updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingDatePolicy {
    FillZero,
    FillPrevious,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResolutionKind {
    Day,
    Week,
    Month,
    Year,
}

impl From<ChartResolution> for ResolutionKind {
    fn from(value: ChartResolution) -> Self {
        match value {
            ChartResolution::Day => ResolutionKind::Day,
            ChartResolution::Week => ResolutionKind::Week,
            ChartResolution::Month => ResolutionKind::Month,
            ChartResolution::Year => ResolutionKind::Year,
        }
    }
}

impl From<ResolutionKind> for ChartResolution {
    fn from(value: ResolutionKind) -> Self {
        match value {
            ResolutionKind::Day => ChartResolution::Day,
            ResolutionKind::Week => ChartResolution::Week,
            ResolutionKind::Month => ChartResolution::Month,
            ResolutionKind::Year => ChartResolution::Year,
        }
    }
}

impl From<ResolutionKind> for String {
    fn from(value: ResolutionKind) -> Self {
        match value {
            ResolutionKind::Day => "DAY",
            ResolutionKind::Week => "WEEK",
            ResolutionKind::Month => "MONTH",
            ResolutionKind::Year => "YEAR",
        }
        .into()
    }
}

pub trait Named {
    /// Name of this data source that represents its contents
    fn name() -> String;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChartKey {
    name: String,
    resolution: ResolutionKind,
}

impl ChartKey {
    pub fn new(name: String, resolution: ResolutionKind) -> Self {
        Self { name, resolution }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn resolution(&self) -> &ResolutionKind {
        &self.resolution
    }

    pub fn as_string(&self) -> String {
        let resolution_string: String = self.resolution.into();
        format!("{}_{}", self.name, resolution_string)
    }
}

impl From<ChartKey> for String {
    fn from(value: ChartKey) -> Self {
        value.as_string()
    }
}

impl Display for ChartKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum IndexingStatus {
    NoneIndexed,
    BlocksIndexed,
    /// It means that blocks are indexed as well
    InternalTransactionsIndexed,
}

impl IndexingStatus {
    // constants for status itself
    pub const MIN: IndexingStatus = IndexingStatus::NoneIndexed;
    pub const MAX: IndexingStatus = IndexingStatus::InternalTransactionsIndexed;
    // constants corresponding to status requirement
    pub const LEAST_RESTRICTIVE: IndexingStatus = Self::MIN;
    pub const MOST_RESTRICTIVE: IndexingStatus = Self::MAX;

    pub fn most_restrictive_from(
        requrements: impl Iterator<Item = IndexingStatus>,
    ) -> IndexingStatus {
        requrements.max().unwrap_or(Self::LEAST_RESTRICTIVE)
    }
}

#[portrait::make(import(
    crate::charts::chart::{
        MissingDatePolicy, IndexingStatus, ResolutionKind, ChartKey
    },
    entity::sea_orm_active_enums::ChartType,
))]
pub trait ChartProperties: Sync + Named {
    /// Combination name + resolution must be unique for each chart
    type Resolution: Timespan + ApproxUnsignedDiff;

    fn chart_type() -> ChartType;
    fn resolution() -> ResolutionKind {
        Self::Resolution::enum_variant()
    }

    /// Expected but not guaranteed to be unique for each chart
    fn key() -> ChartKey {
        ChartKey {
            name: Self::name(),
            resolution: Self::resolution(),
        }
    }

    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillZero
    }

    /// Indexing status at least required by this data source.
    ///
    /// Note that in current implementation this requirement
    /// is propagated to its dependants.
    fn indexing_status_requirement() -> IndexingStatus {
        // most of the charts need indexed blocks
        IndexingStatus::BlocksIndexed
    }

    /// Number of last values that are considered approximate.
    /// (ordered by time)
    ///
    /// E.g. how many end values should be recalculated on (kinda)
    /// lazy update (where `get_update_start` is retrieved successfully).
    /// Also controls marking points as approximate when returning data.
    ///
    /// ## Value
    ///
    /// Usually set to 1 for line charts. Also, data for portion of the
    /// (latest) timeframe has to be recalculated on the next timespan.
    ///
    /// I.e. for number of blocks per day, stats for current day (0) are
    /// not complete because blocks will be produced till the end of the day.
    /// ```text
    ///    |===|=  |
    /// day -1   0
    /// ```
    ///
    /// ## Edge case
    ///
    /// If an update time is exactly at the start of timespan (e.g. midnight),
    /// one less point is considered approximate, because we've got full data
    /// for one timespan.
    fn approximate_trailing_points() -> u64 {
        if Self::chart_type() == ChartType::Counter {
            // there's only one value in counter
            0
        } else {
            1
        }
    }
}

#[macro_export]
macro_rules! define_and_impl_resolution_properties {
    (
        define_and_impl: {
            $($type_name:ident : $res:ty),+
            $(,)?
        },
        base_impl: $source_type:ty $(,)?
    ) => {
        $(
        pub struct $type_name;

        impl $crate::charts::Named for $type_name {
            fn name() -> ::std::string::String {
                <$source_type as $crate::charts::Named>::name()
            }
        }

        #[portrait::fill(portrait::delegate($source_type))]
        impl $crate::charts::ChartProperties for $type_name {
            type Resolution = $res;

            fn resolution() -> $crate::charts::ResolutionKind {
                use $crate::types::Timespan;

                <Self as $crate::charts::ChartProperties>::Resolution::enum_variant()
            }

            fn key() -> $crate::charts::ChartKey {
                $crate::charts::ChartKey::new(Self::name(), Self::resolution())
            }
        }
        )+
    };
}

/// Dynamic object representing a chart
#[derive(Debug)]
pub struct ChartObject {
    pub properties: ChartPropertiesObject,
    pub type_specifics: ChartTypeSpecifics,
}

impl ChartObject {
    pub fn construct_from_chart<T>(t: T) -> Self
    where
        T: ChartProperties + QuerySerialized + Send + 'static,
        QuerySerializedDyn<T::Output>: Into<ChartTypeSpecifics>,
    {
        let type_specifics = <QuerySerializedDyn<T::Output> as Into<ChartTypeSpecifics>>::into(
            std::sync::Arc::new(Box::new(t)),
        );
        assert_eq!(
            type_specifics.as_chart_type(),
            T::chart_type(),
            "data returned by chart {} does not match chart type",
            T::name()
        );
        Self {
            properties: ChartPropertiesObject::construct_from_chart::<T>(),
            type_specifics,
        }
    }
}

/// Dynamic version of trait `ChartProperties`.
///
/// Helpful when need a unified type for different charts
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartPropertiesObject {
    /// unique identifier of the chart
    pub key: ChartKey,
    pub name: String,
    pub resolution: ResolutionKind,
    pub missing_date_policy: MissingDatePolicy,
    pub indexing_status_requirement: IndexingStatus,
    pub approximate_trailing_points: u64,
}

impl ChartPropertiesObject {
    pub fn construct_from_chart<T: ChartProperties>() -> Self {
        Self {
            key: T::key(),
            name: T::name(),
            resolution: T::resolution(),
            missing_date_policy: T::missing_date_policy(),
            indexing_status_requirement: T::indexing_status_requirement(),
            approximate_trailing_points: T::approximate_trailing_points(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::IndexingStatus;

    #[test]
    fn indexing_status_requirements_are_combined_correctly() {
        assert_eq!(
            IndexingStatus::most_restrictive_from(
                vec![
                    IndexingStatus::BlocksIndexed,
                    IndexingStatus::InternalTransactionsIndexed,
                    IndexingStatus::BlocksIndexed
                ]
                .into_iter()
            ),
            IndexingStatus::InternalTransactionsIndexed
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(
                vec![
                    IndexingStatus::NoneIndexed,
                    IndexingStatus::InternalTransactionsIndexed,
                ]
                .into_iter()
            ),
            IndexingStatus::InternalTransactionsIndexed
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(
                vec![
                    IndexingStatus::InternalTransactionsIndexed,
                    IndexingStatus::InternalTransactionsIndexed,
                ]
                .into_iter()
            ),
            IndexingStatus::InternalTransactionsIndexed
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(vec![].into_iter()),
            IndexingStatus::NoneIndexed
        );
    }
}
