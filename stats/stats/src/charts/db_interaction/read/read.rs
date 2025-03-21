use crate::{
    charts::{chart::ChartMetadata, ChartKey},
    data_source::{
        kinds::{local_db::parameter_traits::QueryBehaviour, remote_db::RemoteQueryBehaviour},
        UpdateContext,
    },
    missing_date::{fill_and_filter_chart, fit_into_range},
    range::{exclusive_range_to_inclusive, UniversalRange},
    types::{
        timespans::{DateValue, Month, Week, Year},
        ExtendedTimespanValue, Timespan, TimespanDuration, TimespanValue,
    },
    ChartError, ChartProperties, MissingDatePolicy,
};

use blockscout_db::entity::blocks;
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use entity::{
    chart_data, charts,
    sea_orm_active_enums::{ChartResolution, ChartType},
};
use itertools::Itertools;
use sea_orm::{
    sea_query::{self, Expr},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Statement,
};
use std::{fmt::Debug, ops::Range};
use thiserror::Error;
use tracing::instrument;
