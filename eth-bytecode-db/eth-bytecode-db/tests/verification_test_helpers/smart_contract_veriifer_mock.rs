#![allow(dead_code)]

use mockall::mock;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    solidity_verifier_server::{SolidityVerifier, SolidityVerifierServer},
    vyper_verifier_server::{VyperVerifier, VyperVerifierServer},
    ListVersionsRequest, ListVersionsResponse, VerifyResponse, VerifySolidityMultiPartRequest,
    VerifySolidityStandardJsonRequest, VerifyVyperMultiPartRequest,
};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tonic::transport::Server;

mock! {
    pub SolidityVerifierService {}

    #[async_trait::async_trait]
    impl SolidityVerifier for SolidityVerifierService {
        async fn verify_multi_part(&self, request: tonic::Request<VerifySolidityMultiPartRequest>) -> Result<tonic::Response<VerifyResponse>, tonic::Status>;

        async fn verify_standard_json(&self, request: tonic::Request<VerifySolidityStandardJsonRequest>) -> Result<tonic::Response<VerifyResponse>, tonic::Status>;

        async fn list_versions(&self, request: tonic::Request<ListVersionsRequest>) -> Result<tonic::Response<ListVersionsResponse>, tonic::Status>;
    }
}

mock! {
    #[derive(Clone)]
    pub VyperVerifierService {}

    #[async_trait::async_trait]
    impl VyperVerifier for VyperVerifierService {
        async fn verify_multi_part(&self, request: tonic::Request<VerifyVyperMultiPartRequest>) -> Result<tonic::Response<VerifyResponse>, tonic::Status>;

        async fn list_versions(&self, request: tonic::Request<ListVersionsRequest>) -> Result<tonic::Response<ListVersionsResponse>, tonic::Status>;
    }
}

#[derive(Default)]
pub struct SmartContractVerifierServer {
    solidity_service: Option<MockSolidityVerifierService>,
    vyper_service: Option<MockVyperVerifierService>,
}

impl SmartContractVerifierServer {
    pub fn new() -> Self {
        Self {
            solidity_service: None,
            vyper_service: None,
        }
    }

    pub fn solidity_service(mut self, solidity_service: MockSolidityVerifierService) -> Self {
        self.solidity_service = Some(solidity_service);
        self
    }

    pub fn vyper_service(mut self, vyper_service: MockVyperVerifierService) -> Self {
        self.vyper_service = Some(vyper_service);
        self
    }

    pub async fn start(self) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            Server::builder()
                .add_optional_service(self.solidity_service.map(SolidityVerifierServer::new))
                .add_optional_service(self.vyper_service.map(VyperVerifierServer::new))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });

        addr
    }
}
