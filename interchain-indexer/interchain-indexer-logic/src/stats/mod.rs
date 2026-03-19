//! Statistics orchestration ([`StatsService`]) and batch projection into stats tables.

pub(crate) mod projection;
mod service;

pub use service::StatsService;
