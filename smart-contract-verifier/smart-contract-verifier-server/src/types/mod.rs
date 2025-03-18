mod errors;
mod solidity_multi_part;
mod solidity_standard_json;
mod source;
mod sourcify;
mod sourcify_from_etherscan;
mod verify_response;
mod vyper_multi_part;
mod vyper_standard_json;
pub mod zksolc_standard_json;

pub mod batch_verification;
mod lookup_methods;

pub use self::sourcify::VerifySourcifyRequestWrapper;
pub use errors::StandardJsonParseError;
pub use lookup_methods::{LookupMethodsRequestWrapper, LookupMethodsResponseWrapper};
pub use solidity_multi_part::VerifySolidityMultiPartRequestWrapper;
pub use solidity_standard_json::VerifySolidityStandardJsonRequestWrapper;
pub use sourcify_from_etherscan::VerifyFromEtherscanSourcifyRequestWrapper;
pub use verify_response::VerifyResponseWrapper;
pub use vyper_multi_part::VerifyVyperMultiPartRequestWrapper;
pub use vyper_standard_json::VerifyVyperStandardJsonRequestWrapper;

/********************************************/

pub fn from_hex(value: &str) -> Result<Vec<u8>, hex::FromHexError> {
    if let Some(value) = value.strip_prefix("0x") {
        hex::decode(value)
    } else {
        hex::decode(value)
    }
}

pub fn to_hex(value: Vec<u8>) -> String {
    format!("0x{}", hex::encode(value))
}
