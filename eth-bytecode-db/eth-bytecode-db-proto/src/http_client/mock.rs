use crate::blockscout::eth_bytecode_db::v2::{
    database_actix::route_database, database_server::Database,
    solidity_verifier_actix::route_solidity_verifier, solidity_verifier_server::SolidityVerifier,
    sourcify_verifier_actix::route_sourcify_verifier, sourcify_verifier_server::SourcifyVerifier,
    verifier_alliance_actix::route_verifier_alliance, verifier_alliance_server::VerifierAlliance,
    vyper_verifier_actix::route_vyper_verifier, vyper_verifier_server::VyperVerifier,
    AllianceStats, BatchSearchEventDescriptionsRequest, BatchSearchEventDescriptionsResponse,
    GetAllianceStatsRequest, ListCompilerVersionsRequest, ListCompilerVersionsResponse,
    SearchAllSourcesRequest, SearchAllSourcesResponse, SearchAllianceSourcesRequest,
    SearchEventDescriptionsRequest, SearchEventDescriptionsResponse, SearchSourcesRequest,
    SearchSourcesResponse, SearchSourcifySourcesRequest, VerifierAllianceBatchImportResponse,
    VerifierAllianceBatchImportSolidityMultiPartRequest,
    VerifierAllianceBatchImportSolidityStandardJsonRequest, VerifyFromEtherscanSourcifyRequest,
    VerifyResponse, VerifySolidityMultiPartRequest, VerifySolidityStandardJsonRequest,
    VerifySourcifyRequest, VerifyVyperMultiPartRequest, VerifyVyperStandardJsonRequest,
};
use mockall::mock;
use std::{net::SocketAddr, sync::Arc};

mock! {
    pub DatabaseService {}

    #[async_trait::async_trait]
    impl Database for DatabaseService {
        async fn search_sources(&self, request: tonic::Request<SearchSourcesRequest>) -> Result<tonic::Response<SearchSourcesResponse>, tonic::Status>;

        async fn search_sourcify_sources(&self, request: tonic::Request<SearchSourcifySourcesRequest>) -> Result<tonic::Response<SearchSourcesResponse>, tonic::Status>;

        async fn search_alliance_sources(&self, request: tonic::Request<SearchAllianceSourcesRequest>) -> Result<tonic::Response<SearchSourcesResponse>, tonic::Status>;

        async fn search_all_sources(&self, request: tonic::Request<SearchAllSourcesRequest>) -> Result<tonic::Response<SearchAllSourcesResponse>, tonic::Status>;

        async fn search_event_descriptions(&self, request: tonic::Request<SearchEventDescriptionsRequest>) -> Result<tonic::Response<SearchEventDescriptionsResponse>, tonic::Status>;

        async fn batch_search_event_descriptions(&self, request: tonic::Request<BatchSearchEventDescriptionsRequest>) -> Result<tonic::Response<BatchSearchEventDescriptionsResponse>, tonic::Status>;

        async fn get_alliance_stats(&self, request: tonic::Request<GetAllianceStatsRequest>) -> Result<tonic::Response<AllianceStats>, tonic::Status>;
    }
}

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

mock! {
    #[derive(Clone)]
    pub VerifierAllianceService {}

    #[async_trait::async_trait]
    impl VerifierAlliance for VerifierAllianceService {
        async fn batch_import_solidity_multi_part(&self, request: tonic::Request<VerifierAllianceBatchImportSolidityMultiPartRequest>) -> Result<tonic::Response<VerifierAllianceBatchImportResponse>, tonic::Status>;

        async fn batch_import_solidity_standard_json(&self, request: tonic::Request<VerifierAllianceBatchImportSolidityStandardJsonRequest>) -> Result<tonic::Response<VerifierAllianceBatchImportResponse>, tonic::Status>;
    }
}

#[derive(Default, Clone)]
pub struct EthBytecodeDbServer {
    database_service: Option<Arc<MockDatabaseService>>,
    solidity_verifier_service: Option<Arc<MockSolidityVerifierService>>,
    vyper_verifier_service: Option<Arc<MockVyperVerifierService>>,
    sourcify_verifier_service: Option<Arc<MockSourcifyVerifierService>>,
    verifier_alliance_service: Option<Arc<MockVerifierAllianceService>>,
}

impl EthBytecodeDbServer {
    pub fn new() -> Self {
        Self {
            database_service: None,
            solidity_verifier_service: None,
            vyper_verifier_service: None,
            sourcify_verifier_service: None,
            verifier_alliance_service: None,
        }
    }

    pub fn database_service(mut self, database_service: MockDatabaseService) -> Self {
        self.database_service = Some(Arc::new(database_service));
        self
    }

    pub fn solidity_service(
        mut self,
        solidity_verifier_service: MockSolidityVerifierService,
    ) -> Self {
        self.solidity_verifier_service = Some(Arc::new(solidity_verifier_service));
        self
    }

    pub fn vyper_service(mut self, vyper_verifier_service: MockVyperVerifierService) -> Self {
        self.vyper_verifier_service = Some(Arc::new(vyper_verifier_service));
        self
    }

    pub fn sourcify_verifier_service(
        mut self,
        sourcify_verifier_service: MockSourcifyVerifierService,
    ) -> Self {
        self.sourcify_verifier_service = Some(Arc::new(sourcify_verifier_service));
        self
    }

    pub fn verifier_alliance_service(
        mut self,
        verifier_alliance_service: MockVerifierAllianceService,
    ) -> Self {
        self.verifier_alliance_service = Some(Arc::new(verifier_alliance_service));
        self
    }

    pub async fn start(&self) -> SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let database_service = self.database_service.clone();
        let solidity_verifier_service = self.solidity_verifier_service.clone();
        let vyper_verifier_service = self.vyper_verifier_service.clone();
        let sourcify_verifier_service = self.sourcify_verifier_service.clone();
        let verifier_alliance_service = self.verifier_alliance_service.clone();

        let configure_router = move |service_config: &mut actix_web::web::ServiceConfig| {
            if let Some(database) = database_service.clone() {
                service_config.configure(|config| route_database(config, database));
            }
            if let Some(solidity) = solidity_verifier_service.clone() {
                service_config.configure(|config| route_solidity_verifier(config, solidity));
            }
            if let Some(vyper) = vyper_verifier_service.clone() {
                service_config.configure(|config| route_vyper_verifier(config, vyper));
            }
            if let Some(sourcify) = sourcify_verifier_service.clone() {
                service_config.configure(|config| route_sourcify_verifier(config, sourcify));
            }
            if let Some(verifier_alliance) = verifier_alliance_service.clone() {
                service_config
                    .configure(|config| route_verifier_alliance(config, verifier_alliance));
            }
        };

        let server =
            actix_web::HttpServer::new(move || actix_web::App::new().configure(&configure_router))
                .listen(listener)
                .expect("failed to bind server")
                .run();
        tokio::spawn(server);

        addr
    }
}
