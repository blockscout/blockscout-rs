//! To simplify implementation of overly-generic `DataSource` trait
//! as well as to reduce boilerblate, types in this module can
//! be used.
//!
//! Generally, they are represented as types (or type aliases)
//! with generic parameters = parameters for the particular kind.
//!
//! [More details on data sources](crate::data_source)
//!
//! ## Dependency requirements
//! Some data sources have additional constraints on data sources
//! that are not checked in code. The dependency(-ies) might return
//! data with some dates missing. Usually this means either
//! - Value for the date == value for the previous date (==[`MissingDatePolicy::FillPrevious`](crate::charts::MissingDatePolicy))
//! - Value is 0 (==[`MissingDatePolicy::FillZero`](crate::charts::MissingDatePolicy))
//!
//! It's not checked in the code since it is both expected to work
//! for charts with set `MissingDatePolicy` and other sources
//! without such parameters.
//!
//! Implementing this seems like a high-effort job + these errors
//! should be caught by tests very fast (or be obvious in runtime),
//! so it's left as this notice. It should be linked to the data
//! sources' docs where applicable.

pub mod auxiliary;
pub mod data_manipulation;
pub mod local_db;
pub mod remote_db;
