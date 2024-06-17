//! To simplify implementation of overly-generic `DataSource` trait
//! as well as to reduce boilerblate, types in this module can
//! be used.
//!
//! Generally, they are represented as types (or type aliases)
//! with generic parameters = parameters for the particular kind.
//!
//! [More details on data sources](crate::data_source)

pub mod auxiliary;
pub mod data_manipulation;
pub mod local_db;
pub mod remote_db;
