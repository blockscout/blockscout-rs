//! Update logic for charts.
//!
//! Depending on the chart nature, various tactics are better fit (in terms of efficiency, performance, etc.).

use async_trait::async_trait;
use sea_orm::prelude::*;

use crate::{Chart, DateValue, UpdateError};

mod batch;
pub(crate) mod common_operations;
mod dependent;
mod full;
mod partial;

pub use batch::ChartBatchUpdater;
pub use dependent::{last_point, parse_and_cumsum, parse_and_sum, ChartDependentUpdater};
pub use full::ChartFullUpdater;
pub use partial::ChartPartialUpdater;
