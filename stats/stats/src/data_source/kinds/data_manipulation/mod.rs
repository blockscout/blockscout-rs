//! Data sources that manipulate data received from other sources.
//!
//! These sources do not store any state, use [`local_db` sources](super::local_db)
//! for persistency.

pub mod delta;
pub mod last_point;
pub mod map;
pub mod sum_point;
