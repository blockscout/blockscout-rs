mod charts;
pub mod data_processing;
pub mod data_source;
pub mod metrics;
mod missing_date;
pub mod update_group;
pub mod update_groups;
pub(crate) mod utils;

#[cfg(any(feature = "test-utils", test))]
pub mod tests;

pub use entity;
pub use migration;

pub use charts::{
    counters,
    db_interaction::{
        read::{get_line_chart_data, get_raw_counters, ReadError},
        types::{DateValueString, ExtendedDateValue, ZeroDateValue},
    },
    lines, ChartProperties, ChartPropertiesObject, MissingDatePolicy, Named, Point, UpdateError,
};
