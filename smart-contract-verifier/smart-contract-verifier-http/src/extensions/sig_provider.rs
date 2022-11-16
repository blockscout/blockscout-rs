#![allow(dead_code)]

use serde::Deserialize;
use smart_contract_verifier::{Middleware, SourcifySuccess, VerificationSuccess};
use std::net::SocketAddr;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    pub addr: SocketAddr,
}

pub struct SigProvider {}

impl SigProvider {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl Middleware<VerificationSuccess> for SigProvider {
    async fn call(&self, _output: &VerificationSuccess) -> () {
        todo!()
    }
}

#[async_trait::async_trait]
impl Middleware<SourcifySuccess> for SigProvider {
    async fn call(&self, _output: &SourcifySuccess) -> () {
        todo!()
    }
}
