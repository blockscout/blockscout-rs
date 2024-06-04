mod artifacts;
mod batch_contract_verifier;
mod compilation;
mod errors;
mod transformations;
pub mod zksync_batch_contract_verifier;
mod zk_compilation;
mod zk_artifacts;
mod zk_lossless_output;

pub use batch_contract_verifier::{
    verify_solidity, BatchSuccess, Match as BatchMatch,
    VerificationResult as BatchVerificationResult,
};
pub use errors::BatchError;
pub use zksync_batch_contract_verifier::{ZkBatchSuccess, verify_zksolc, VerificationResult as ZkBatchVerificationResult};

#[derive(Debug)]
pub enum VerificationResult<Success> {
    Success(Success),
    Failure(Vec<errors::VerificationError>),
}

pub fn decode_hex(value: &str) -> Result<Vec<u8>, hex::FromHexError> {
    if let Some(value) = value.strip_prefix("0x") {
        hex::decode(value)
    } else {
        hex::decode(value)
    }
}
