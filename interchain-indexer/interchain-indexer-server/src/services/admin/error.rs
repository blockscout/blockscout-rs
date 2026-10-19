// SPDX-License-Identifier: LicenseRef-Blockscout

/// Failure of an admin method, mapped to a gRPC status (and so an HTTP code) at
/// the API boundary.
#[derive(Debug, thiserror::Error)]
pub(crate) enum AdminError {
    #[error("{0}")]
    InvalidArgument(String),
    #[error("{0}")]
    NotFound(String),
    /// The request is well formed but the current state refuses it.
    #[error("{0}")]
    FailedPrecondition(String),
    #[error("{0}")]
    Aborted(String),
    #[error("internal error")]
    Internal(#[from] anyhow::Error),
}

impl From<AdminError> for tonic::Status {
    fn from(err: AdminError) -> Self {
        match err {
            AdminError::InvalidArgument(message) => Self::invalid_argument(message),
            AdminError::NotFound(message) => Self::not_found(message),
            AdminError::FailedPrecondition(message) => Self::failed_precondition(message),
            AdminError::Aborted(message) => Self::aborted(message),
            // Never expose database or provider text; the full error is logged
            // where the request is finished.
            AdminError::Internal(_) => Self::internal("internal server error"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Code;

    #[test]
    fn admin_error_maps_to_status_and_sanitizes_internal() {
        let cases = [
            (
                AdminError::InvalidArgument("bad field".into()),
                Code::InvalidArgument,
                "bad field",
            ),
            (
                AdminError::NotFound("no such asset".into()),
                Code::NotFound,
                "no such asset",
            ),
            (
                AdminError::FailedPrecondition("no checkpoint yet".into()),
                Code::FailedPrecondition,
                "no checkpoint yet",
            ),
            (AdminError::Aborted("retry".into()), Code::Aborted, "retry"),
            (
                AdminError::Internal(anyhow::anyhow!("password=hunter2 at db.internal:5432")),
                Code::Internal,
                "internal server error",
            ),
        ];
        for (err, code, message) in cases {
            let status = tonic::Status::from(err);
            assert_eq!(status.code(), code);
            assert_eq!(status.message(), message);
        }
    }
}
