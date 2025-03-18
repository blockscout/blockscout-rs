//! # Data sources
//!
//! ## Overview
//!
//! Statistics consist of charts (i.e. line charts & counters) and their dependencies (deps).
//! In general case, their relationship can be represented by directed acyclic graph (DAG).
//!
//! One simple example is a chart that uses result of SQL query on remote DB. In this case, DAG
//! will consist of the locally-stored chart and the remote DB + query. In more complex cases,
//! the chart could use data from multiple sources
//! (e.g. `average gas per block (in a day) = total gas per day / blocks per day`)
//!
//! Conveniently, rust types are also composable as DAG (ignoring pointer types that allow recursion).
//! Therefore, each "node" in stats (charts + deps) is represented by a separate type.
//!
//! All these types have common trait ([`DataSource`]) with relationships
//! being associated types ([`MainDependencies`](DataSource::MainDependencies) and
//! [`ResolutionDependencies`](DataSource::ResolutionDependencies)).
//! They can be [initialized](`DataSource::init_recursively`), [updated](DataSource::update_recursively),
//! or [queried](DataSource::query_data).
//!
//! The initialization & update is expected to
//! 1. Trigger the same operation on source's dependencies recursively
//! 2. Perform the action on itself.
//!     This ensures that the data requested by current source from dependencies is relevant.
//!
//! ## Implementation
//!
//! In general, it is easier to use types from [`kinds`] to get a [`DataSource`]. For example,
//! sources with data pulled from external DB should fit [`kinds::remote_db`], and locally-stored
//! charts - [`kinds::local_db`]. More variations can be found in [`kinds`].
//!
//! ## Usage
//!
//! The `DataSource`s methods are not intended to be called directly. Since the charts comprise
//! a DAG, it is usually not possible to traverse (and, thus, update) the connected graph from a single node.
//! We would like to have all nodes in the group with their dependencies to be updated simultaneously
//! (at the same 'update').
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
//! (`C` and `D` both depend on `B`)
//!
//! Furthermore, we would like a possibility to disable some charts (for external use) without
//! affecting others (esp. ones depending on them).
//!
//! Therefore, the charts (`DataSource`s that also implement `ChartProperties`) are combined into
//! [update groups](crate::update_group). In particular, [`SyncUpdateGroup`](crate::update_group::SyncUpdateGroup).
//!
//! To make the 'simultaneous' update work, the update timestamp is recursively passed inside
//! [`UpdateContext`]. This way, SQL queries can use this time to get data relevant at this particular
//! timestamp.
//!
//! See ([module](crate::update_group) & [struct](crate::update_group::SyncUpdateGroup)) documentation for details.

pub mod kinds;
pub mod source;
pub mod types;

#[cfg(test)]
mod tests;

pub use source::DataSource;
pub use types::{UpdateContext, UpdateParameters};
