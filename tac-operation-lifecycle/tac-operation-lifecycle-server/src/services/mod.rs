// SPDX-License-Identifier: LicenseRef-Blockscout

// `tonic::Status` is ~176 bytes, which trips the lint in generated and http-client code.
#![allow(clippy::result_large_err)]

mod health;
mod operations;
mod statistic;

pub use health::HealthService;
pub use operations::OperationsService;
pub use statistic::StatisticService;
