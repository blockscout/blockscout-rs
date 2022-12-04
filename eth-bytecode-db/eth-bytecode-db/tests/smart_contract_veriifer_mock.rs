use tokio::sync::OnceCell;
use mockall::mock;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::solidity_verifier_server::SolidityVerifier;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{ListVersionsRequest, ListVersionsResponse, VerifyResponse, VerifySolidityMultiPartRequest, VerifySolidityStandardJsonRequest, VerifyVyperMultiPartRequest};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::sourcify_verifier_server::SourcifyVerifier;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::vyper_verifier_server::VyperVerifier;

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
    pub VyperVerifierService {}

    #[async_trait::async_trait]
    impl VyperVerifier for VyperVerifierService {
        async fn verify_multi_part(&self, request: tonic::Request<VerifyVyperMultiPartRequest>) -> Result<tonic::Response<VerifyResponse>, tonic::Status>;

        async fn list_versions(&self, request: tonic::Request<ListVersionsRequest>) -> Result<tonic::Response<ListVersionsResponse>, tonic::Status>;
    }
}

async fn global_server() -> &'static AppRouter {
    static SERVER: OnceCell<AppRouter> = OnceCell::const_new();
    APP_ROUTER
        .get_or_init(|| async {
            let mut settings = Settings::default();
            settings.sourcify.enabled = false;
            AppRouter::new(settings)
                .await
                .expect("couldn't initialize the app")
        })
        .await
}


async fn start_server() {

}

