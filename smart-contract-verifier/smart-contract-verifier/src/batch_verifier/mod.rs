mod artifacts;
mod batch_contract_verifier;
mod compilation;
mod errors;
mod transformations;

pub use batch_contract_verifier::{
    verify_solidity, BatchError, BatchSuccess, Match as BatchMatch,
    VerificationResult as BatchVerificationResult,
};

pub fn decode_hex(value: &str) -> Result<Vec<u8>, hex::FromHexError> {
    if let Some(value) = value.strip_prefix("0x") {
        hex::decode(value)
    } else {
        hex::decode(value)
    }
}
