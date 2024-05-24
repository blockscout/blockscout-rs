//!

pub mod kinds;
pub mod source;
mod source_metrics;
pub mod types;

#[cfg(test)]
mod example;

pub use source::DataSource;
pub use types::{UpdateContext, UpdateParameters};
