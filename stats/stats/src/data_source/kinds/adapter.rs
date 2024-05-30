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

pub struct SourceAdapterWrapper<T: SourceAdapter>(PhantomData<T>);

impl<T: SourceAdapter + Named> Named for SourceAdapterWrapper<T> {
    const NAME: &'static str = T::NAME;
}

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

pub struct ParseAdapterWrapper<T: ParseAdapter>(PhantomData<T>);

pub type ParseAdapterDataSourceWrapper<T: ParseAdapter> =
    SourceAdapterWrapper<ParseAdapterWrapper<T>>;

impl<T: ParseAdapter> SourceAdapter for ParseAdapterWrapper<T>
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
