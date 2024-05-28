mod charts;
pub mod data_processing;
pub mod data_source;
pub mod metrics;
mod missing_date;
pub mod update_group;
pub(crate) mod utils;

#[cfg(any(feature = "test-utils", test))]
pub mod tests;

pub use entity;
pub use migration;

pub use charts::{
    counters,
    db_interaction::{
        read::{get_chart_data, get_counters, ReadError},
        types::{DateValue, ExtendedDateValue},
    },
    lines, Chart, ChartDynamic, MissingDatePolicy, UpdateError,
};
