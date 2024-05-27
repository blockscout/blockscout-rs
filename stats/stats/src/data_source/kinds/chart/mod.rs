//! Chart data sources.
//!
//! Charts are data sources that implement `Chart` and (thus) are expected
//! to be delivered/served upon requests.
//!
//! The 'base' case for charts is [`UpdateableChart`].

mod base;
mod batch;
mod remote;

pub use base::{UpdateableChart, UpdateableChartWrapper};
pub use batch::{BatchUpdateableChart, BatchUpdateableChartWrapper};
pub use remote::{RemoteChart, RemoteChartWrapper};
