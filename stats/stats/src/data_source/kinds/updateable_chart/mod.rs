//! Chart data sources.
//!
//! Charts are data sources that implement `Chart` and (thus) are expected
//! to be delivered/served upon requests.
//!
//! The 'base' case for charts is [`UpdateableChart`].

mod base;
pub mod batch;
pub mod clone;
pub mod cumulative;
pub mod delta;
pub mod last_point;
pub mod sum_point;

pub use base::{UpdateableChart, UpdateableChartWrapper};
