mod verification_test_helpers;

use crate::verification_test_helpers::{
    smart_contract_verifer_mock::{
        MockSolidityVerifierService, MockVyperVerifierService, SmartContractVerifierServer,
    },
    VerifierService,
};
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use pretty_assertions::assert_eq;
use rstest::rstest;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;
use tonic::Response;

const TEST_SUITE_NAME: &str = "compiler_versions";

async fn test_versions(
    test_name: &str,
    route: &str,
    verifier: impl VerifierService<smart_contract_verifier_v2::ListCompilerVersionsResponse>,
    verifier_response: smart_contract_verifier_v2::ListCompilerVersionsResponse,
) {
    let db = verification_test_helpers::init_db(TEST_SUITE_NAME, test_name).await;
    let verifier_addr =
        verification_test_helpers::init_verifier_server(verifier, verifier_response.clone()).await;
    let eth_bytecode_db_base =
        verification_test_helpers::init_eth_bytecode_db_server(db.db_url(), verifier_addr).await;

    let response = reqwest::Client::new()
        .get(eth_bytecode_db_base.join(route).unwrap())
        .send()
        .await
        .expect("Failed to send request");

    // Assert that status code is success
    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await.expect("Read body as text");
        panic!("Invalid status code (success expected). Status: {status}. Message: {message}")
    }

    let response: eth_bytecode_db_v2::ListCompilerVersionsResponse = response
        .json()
        .await
        .expect("Response deserialization failed");

    assert_eq!(
        verifier_response.compiler_versions, response.compiler_versions,
        "Compiler versions mismatch"
    );
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn solidity() {
    const TEST_NAME: &str = "solidity";

    impl VerifierService<smart_contract_verifier_v2::ListCompilerVersionsResponse>
        for MockSolidityVerifierService
    {
        fn add_into_service(
            &mut self,
            response: smart_contract_verifier_v2::ListCompilerVersionsResponse,
        ) {
            self.expect_list_compiler_versions()
                .returning(move |_| Ok(Response::new(response.clone())));
        }

        fn build_server(self) -> SmartContractVerifierServer {
            SmartContractVerifierServer::new().solidity_service(self)
        }
    }

    let route = "/api/v2/verifier/solidity/versions";
    let verifier = MockSolidityVerifierService::new();
    let verifier_response = smart_contract_verifier_v2::ListCompilerVersionsResponse {
        compiler_versions: vec![
            "v0.5.11+commit.22be8592".into(),
            "v0.6.7+commit.b8d736ae".into(),
            "v0.8.7+commit.e28d00a7".into(),
        ],
    };
    test_versions(TEST_NAME, route, verifier, verifier_response).await;
}
#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn vyper() {
    const TEST_NAME: &str = "vyper";

    impl VerifierService<smart_contract_verifier_v2::ListCompilerVersionsResponse>
        for MockVyperVerifierService
    {
        fn add_into_service(
            &mut self,
            response: smart_contract_verifier_v2::ListCompilerVersionsResponse,
        ) {
            self.expect_list_compiler_versions()
                .returning(move |_| Ok(Response::new(response.clone())));
        }

        fn build_server(self) -> SmartContractVerifierServer {
            SmartContractVerifierServer::new().vyper_service(self)
        }
    }

    let route = "/api/v2/verifier/vyper/versions";
    let verifier = MockVyperVerifierService::new();
    let verifier_response = smart_contract_verifier_v2::ListCompilerVersionsResponse {
        compiler_versions: vec![
            "v0.3.1+commit.0463ea4c".into(),
            "v0.3.6+commit.4a2124d0".into(),
        ],
    };
    test_versions(TEST_NAME, route, verifier, verifier_response).await;
}
