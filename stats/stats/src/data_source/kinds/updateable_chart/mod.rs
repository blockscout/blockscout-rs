//! Chart data sources.
//!
//! Charts are data sources that implement `Chart` and (thus) are expected
//! to be delivered/served upon requests.
//!
//! The 'base' case for charts is [`UpdateableChart`].

mod base;
pub mod batch;

pub use base::{UpdateableChart, UpdateableChartDataSourceWrapper};
