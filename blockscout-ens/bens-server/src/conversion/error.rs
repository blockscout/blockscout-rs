use crate::conversion::ConversionError;
use bens_logic::{protocols::ProtocolError, subgraph::SubgraphReadError};

pub fn map_subgraph_error(err: SubgraphReadError) -> tonic::Status {
    match err {
        SubgraphReadError::Protocol(err) => map_protocol_error(err),
        SubgraphReadError::DbErr(_) | SubgraphReadError::Internal(_) => {
            tracing::error!(err =? err, "error during request handle");
            tonic::Status::internal("internal error")
        }
    }
}

pub fn map_protocol_error(err: ProtocolError) -> tonic::Status {
    match err {
        ProtocolError::InvalidName { .. } => tonic::Status::invalid_argument(err.to_string()),
        ProtocolError::ProtocolNotFound(_) => tonic::Status::not_found(err.to_string()),
        ProtocolError::NetworkNotFound(_) => tonic::Status::not_found(err.to_string()),
        ProtocolError::Internal(_) => {
            tracing::error!(err =? err, "error during request handle");
            tonic::Status::internal("internal error")
        }
        ProtocolError::TooManyProtocols { .. } => tonic::Status::invalid_argument(err.to_string()),
    }
}

pub fn map_convertion_error(err: ConversionError) -> tonic::Status {
    match err {
        ConversionError::UserRequest(_) => tonic::Status::invalid_argument(err.to_string()),
        ConversionError::LogicOutput(_) => tonic::Status::internal(err.to_string()),
    }
}
