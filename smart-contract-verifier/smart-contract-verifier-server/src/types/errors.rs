// SPDX-License-Identifier: LicenseRef-Blockscout

use thiserror::Error;
use tonic::Status;

/// Logs an internal failure and answers with a fixed message. The cause can name private
/// infrastructure, such as the compiler host's address, so it never reaches the client.
pub fn internal_error_status(error: &dyn std::fmt::Debug) -> Status {
    tracing::error!(err = format!("{error:#?}"), "internal error");
    Status::internal("internal error")
}

#[derive(Error, Debug)]
pub enum StandardJsonParseError {
    #[error("content is not a valid standard json: {0}")]
    InvalidContent(#[from] serde_json::Error),
    #[error("{0}")]
    BadRequest(#[from] anyhow::Error),
}
