//! # Data sources
//!
//! ## Overview
//!
//! Statistics consist of charts (i.e. line charts & counters) and their dependencies (deps).
//! In general case, their relationship can be represented by directed acyclic graph (DAG).
//! Conveniently, rust types are also composable as DAG (ignoring pointer types that allow recursion).
//! Therefore, each "node" in stats (charts + deps) is represented by a separate type.
//!
//! These types have common trait ([`DataSource`]) with relationships being represented
//! as associated types ([`PrimaryDependency`](DataSource::PrimaryDependency) and
//! [`SecondaryDependencies`](DataSource::SecondaryDependencies)).
//! They can be [initialized](`DataSource::init_recursively`), [updated](DataSource::update_recursively),
//! or [queried](DataSource::query_data).
//!
//! The initialization & update is expected to
//! 1. Trigger the same operation on source's dependencies recursively
//! 2. Perform the action on itself.
//! This ensures that the data requested by current source from dependencies is relevant.
//!
//! ## Other source kinds
//!
//! Apart from the base trait, there are multiple special cases that simplify implementation.
//! For example:
//! - [`RemoteSource`](`kinds::remote::RemoteSource`) - data pulled from external (DB)
//! - [`UpdateableChart`](`kinds::updateable_chart::UpdateableChart`) - any chart (stored in local (stats) DB)
//!
//! See [`kinds`] and respective docs for more info
//!
//! ## Implementation
//!
//! Usually, it should be much easier to implement special kind of data source (see above).
//!
//! The approximate workflow is the following:
//! 1. Create newtype `SomeType` that will represent your data source-ish (name it accordingly).
//! 2. Implement `TraitName` for `SomeType`.
//! 3. Use `TraitNameWrapper<SomeType>` to use it as `DataSource`.
//!
//! ### On wrappers
//!
//! Wrappers (placed together with 'special' traits) are used to get type that implements `DataSource`.
//! For example, to get `DataSource` from type `T` implementing `RemoteSource`, one can use
//! `RemoteSourceWrapper<T>`.
//!
//! Generally, the idea is to have `TraitNameWrapper` for each helper/special case trait `TraitName`.
//!
//! These newtypes-wrappers are a 'workaround' for implementation conflicts (i.e. they indicate
//! some structure/relationship between the 'special' traits).
//!
//! ### Manual implementation
//!
//! When implementing the trait manually, consider the following:
//! - You probably want to implement `DataSourceMetrics` or find some other way to
//! produce [`CHART_FETCH_NEW_DATA_TIME`](crate::metrics::CHART_FETCH_NEW_DATA_TIME) metric
//!
//! ## Usage
//!
//! The `DataSource`s methods are not intended to be called directly. Since the charts comprise
//! a DAG, it is not possible to traverse (i.e. update) the connected graph from a single node.
//! We would like to have all 'source' nodes (w/o dependants) to be updated simultaneously.
//!
//! For example, it makes sense to update C and D together, since in this case it's possible to
//! 'reuse' update of A and B:
//!
//! ```text
//! A ┌─►B
//! ▲ │  ▲
//! ├─┘  │
//! C    D
//! ```
//!
//! Furthermore, we would like a possibility to disable some charts (for external use) without
//! affecting others (esp. ones depending on them).
//!
//! Therefore, the charts (`DataSource`s that also implement `Chart`) are combined into
//! [update groups](crate::update_group). In particular, [`SyncUpdateGroup`](crate::update_group::SyncUpdateGroup).
//! See (module & struct) documentation for details.

pub mod kinds;
pub mod source;
mod source_metrics;
pub mod types;

#[cfg(test)]
mod example;

pub use source::DataSource;
pub use types::{UpdateContext, UpdateParameters};
