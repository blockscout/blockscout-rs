//! Data source adapter.
//! Allows manipulating data read from the inner
//! data source.
//!
//! Kinda like `map` for a data source. I.e. applies
//! a function to the output.

use std::{fmt::Display, marker::PhantomData, ops::RangeInclusive, str::FromStr};

use blockscout_metrics_tools::AggregateTimer;
use chrono::Utc;
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbErr};

use crate::{
    charts::db_interaction::types::DateValue,
    data_source::{DataSource, UpdateContext},
    DateValueString, Named, UpdateError,
};

pub trait SourceAdapter {
    type InnerSource: DataSource;
    type Output: Send;
    fn function(
        inner_data: <Self::InnerSource as DataSource>::Output,
    ) -> Result<Self::Output, UpdateError>;
}

/// Wrapper to convert type implementing [`SourceAdapter`] to another implementing [`DataSource`]
pub struct SourceAdapterWrapper<T: SourceAdapter>(PhantomData<T>);

impl<T: SourceAdapter> DataSource for SourceAdapterWrapper<T> {
    type PrimaryDependency = T::InnerSource;
    type SecondaryDependencies = ();
    type Output = T::Output;

    // Adapter by itself does not store anything
    const MUTEX_ID: Option<&'static str> = None;

    async fn init_itself(
        _db: &DatabaseConnection,
        _init_time: &chrono::DateTime<Utc>,
    ) -> Result<(), DbErr> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn update_itself(_cx: &UpdateContext<'_>) -> Result<(), UpdateError> {
        // just an adapter; inner is handled recursively
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: Option<RangeInclusive<DateTimeUtc>>,
        remote_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, UpdateError> {
        let inner_data =
            <T::InnerSource as DataSource>::query_data(cx, range, remote_fetch_timer).await?;
        let transformed = T::function(inner_data)?;
        Ok(transformed)
    }
}

pub trait ParseAdapter {
    type InnerSource: DataSource<Output = Vec<DateValueString>> + Named;
    type ParseInto: DateValue + Send;
}

/// Wrapper to convert type implementing [`ParseAdapter`] to another that implements [`DataSource`]
pub type ParseAdapterWrapper<T> = SourceAdapterWrapper<ParseAdapterLocalWrapper<T>>;

/// Wrapper to get type implementing "parent" trait. Use [`ParseAdapterWrapper`] to get [`DataSource`]
pub struct ParseAdapterLocalWrapper<T: ParseAdapter>(PhantomData<T>);

impl<T: ParseAdapter> SourceAdapter for ParseAdapterLocalWrapper<T>
where
    <T::ParseInto as DateValue>::Value: FromStr,
    <<T::ParseInto as DateValue>::Value as FromStr>::Err: Display,
{
    type InnerSource = T::InnerSource;
    type Output = Vec<T::ParseInto>;

    fn function(inner_data: Vec<DateValueString>) -> Result<Self::Output, UpdateError> {
        let parsed_data = inner_data
            .into_iter()
            .map(|p| {
                let (date, val_str) = p.into_parts();
                let val_parsed = val_str
                    .parse::<<T::ParseInto as DateValue>::Value>()
                    .map_err(|e| {
                        UpdateError::Internal(format!(
                            "failed to parse values in chart '{}': {e}",
                            T::InnerSource::NAME
                        ))
                    })?;
                Ok(T::ParseInto::from_parts(date, val_parsed))
            })
            .collect::<Result<Vec<T::ParseInto>, UpdateError>>()?;
        Ok(parsed_data)
    }
}

pub trait ToStringAdapter {
    type InnerSource: DataSource<Output = Vec<Self::ConvertFrom>>;
    type ConvertFrom: Into<DateValueString>;
}

/// Wrapper to convert type implementing [`ToStringAdapter`] to another that implements [`DataSource`]
pub type ToStringAdapterWrapper<T> = SourceAdapterWrapper<ToStringAdapterLocalWrapper<T>>;

/// Wrapper to get type implementing "parent" trait. Use [`ToStringAdapterWrapper`] to get [`DataSource`]
pub struct ToStringAdapterLocalWrapper<T>(PhantomData<T>);

impl<T: ToStringAdapter> SourceAdapter for ToStringAdapterLocalWrapper<T> {
    type InnerSource = T::InnerSource;
    type Output = Vec<DateValueString>;

    fn function(
        inner_data: <Self::InnerSource as DataSource>::Output,
    ) -> Result<Self::Output, UpdateError> {
        Ok(inner_data.into_iter().map(|p| p.into()).collect())
    }
}
