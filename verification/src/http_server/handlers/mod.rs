use serde::Serialize;

pub mod compilation;
pub mod status;
pub mod verification;

#[derive(Debug, Serialize, PartialEq)]
pub struct VerificationResponse {
    pub verified: bool,
}
