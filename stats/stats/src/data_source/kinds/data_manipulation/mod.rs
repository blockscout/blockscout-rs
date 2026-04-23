//! Data sources that manipulate data received from other sources.
//!
//! These sources do not store any state, use [`local_db` sources](super::local_db)
//! for persistent sources that retrieve local data on query.

pub mod delta;
pub mod filter_deducible;
pub mod last_point;
pub mod map;
pub mod resolutions;
pub mod sum_point;
