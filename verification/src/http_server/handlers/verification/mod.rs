use serde::Serialize;

pub mod routes;
pub mod solidity;
pub mod sourcify;

#[derive(Debug, Serialize, PartialEq)]
pub struct VerificationResponse {
    pub verified: bool,
}
