//! Basically the same as normal config but without lists.
//! Lists are currently not supported by `config` crate with environmental vars.
//!
//! Instead, we have the same items but with `order` field that defines relative position between them.

pub mod charts;
pub mod update_schedule;
