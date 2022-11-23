#[cfg(feature = "ethers-core")]
pub use ethers_core::types::Bytes;

#[cfg(not(feature = "ethers-core"))]
mod bytes;
#[cfg(not(feature = "ethers-core"))]
pub use crate::bytes::Bytes;
