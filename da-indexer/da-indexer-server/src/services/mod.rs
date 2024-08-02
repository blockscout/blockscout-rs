mod celestia;
mod eigenda;
mod health;

pub use celestia::CelestiaService;
pub use eigenda::EigenDaService;
pub use health::HealthService;

use base64::prelude::*;
use blockscout_display_bytes::Bytes;
use std::str::FromStr;
use tonic::Status;

pub fn bytes_from_hex_or_base64(s: &str, name: &str) -> Result<Vec<u8>, Status> {
    Bytes::from_str(s)
        .map(|b| b.to_vec())
        .or_else(|_| BASE64_STANDARD.decode(s))
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to decode {}", name);
            Status::invalid_argument(format!("failed to decode {}", name))
        })
}
