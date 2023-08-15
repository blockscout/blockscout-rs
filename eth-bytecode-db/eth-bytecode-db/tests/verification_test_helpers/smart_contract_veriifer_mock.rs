#![allow(dead_code)]

use mockall::mock;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    solidity_verifier_server::{SolidityVerifier, SolidityVerifierServer},
    sourcify_verifier_server::{SourcifyVerifier, SourcifyVerifierServer},
    vyper_verifier_server::{VyperVerifier, VyperVerifierServer},
    ListCompilerVersionsRequest, ListCompilerVersionsResponse, VerifyFromEtherscanSourcifyRequest,
    VerifyResponse, VerifySolidityMultiPartRequest, VerifySolidityStandardJsonRequest,
    VerifySourcifyRequest, VerifyVyperMultiPartRequest, VerifyVyperStandardJsonRequest,
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

        async fn list_compiler_versions(&self, request: tonic::Request<ListCompilerVersionsRequest>) -> Result<tonic::Response<ListCompilerVersionsResponse>, tonic::Status>;
    }
}

mock! {
    #[derive(Clone)]
    pub VyperVerifierService {}

    #[async_trait::async_trait]
    impl VyperVerifier for VyperVerifierService {
        async fn verify_multi_part(&self, request: tonic::Request<VerifyVyperMultiPartRequest>) -> Result<tonic::Response<VerifyResponse>, tonic::Status>;

        async fn verify_standard_json(&self, request: tonic::Request<VerifyVyperStandardJsonRequest>) -> Result<tonic::Response<VerifyResponse>, tonic::Status>;

        async fn list_compiler_versions(&self, request: tonic::Request<ListCompilerVersionsRequest>) -> Result<tonic::Response<ListCompilerVersionsResponse>, tonic::Status>;
    }
}

mock! {
    #[derive(Clone)]
    pub SourcifyVerifierService {}

    #[async_trait::async_trait]
    impl SourcifyVerifier for SourcifyVerifierService {
        async fn verify(&self, request: tonic::Request<VerifySourcifyRequest>) -> Result<tonic::Response<VerifyResponse>, tonic::Status>;

        async fn verify_from_etherscan(&self, request: tonic::Request<VerifyFromEtherscanSourcifyRequest>) -> Result<tonic::Response<VerifyResponse>, tonic::Status>;
    }
}

#[derive(Default)]
pub struct SmartContractVerifierServer {
    solidity_service: Option<MockSolidityVerifierService>,
    vyper_service: Option<MockVyperVerifierService>,
    sourcify_service: Option<MockSourcifyVerifierService>,
}

impl SmartContractVerifierServer {
    pub fn new() -> Self {
        Self {
            solidity_service: None,
            vyper_service: None,
            sourcify_service: None,
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

    pub fn sourcify_service(mut self, sourcify_service: MockSourcifyVerifierService) -> Self {
        self.sourcify_service = Some(sourcify_service);
        self
    }

    pub async fn start(self) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            Server::builder()
                .add_optional_service(self.solidity_service.map(SolidityVerifierServer::new))
                .add_optional_service(self.vyper_service.map(VyperVerifierServer::new))
                .add_optional_service(self.sourcify_service.map(SourcifyVerifierServer::new))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });

        addr
    }
}
