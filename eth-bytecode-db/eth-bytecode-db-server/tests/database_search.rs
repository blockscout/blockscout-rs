mod verification_test_helpers;

use crate::verification_test_helpers::{
    init_db, init_eth_bytecode_db_server, init_verifier_server,
};
use async_trait::async_trait;
use eth_bytecode_db::{verification, verification::MatchType};
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::{
    v2 as eth_bytecode_db_v2,
    v2::{SearchSourcesResponse, SearchSourcifySourcesRequest, Source},
};
use pretty_assertions::assert_eq;
use rstest::{fixture, rstest};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;
use std::collections::BTreeMap;
use tonic::Response;
use verification_test_helpers::{
    smart_contract_verifer_mock::{MockSolidityVerifierService, SmartContractVerifierServer},
    test_input_data, VerifierService,
};

const TEST_SUITE_NAME: &str = "database_search";

#[async_trait]
impl VerifierService<smart_contract_verifier_v2::VerifyResponse> for MockSolidityVerifierService {
    fn add_into_service(&mut self, response: smart_contract_verifier_v2::VerifyResponse) {
        self.expect_verify_multi_part()
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().solidity_service(self)
    }
}

#[fixture]
fn service() -> MockSolidityVerifierService {
    MockSolidityVerifierService::new()
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn search_sourcify_sources(service: MockSolidityVerifierService) {
    const ROUTE: &str = "/api/v2/bytecodes/sources:search-sourcify";

    let db = init_db(TEST_SUITE_NAME, "test_returns_valid_source").await;

    let test_data = test_input_data::basic(verification::SourceType::Solidity, MatchType::Partial);

    let db_url = db.db_url();
    let verifier_addr = init_verifier_server(service, test_data.verifier_response).await;

    let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

    let chain_id = "5".to_string();
    let contract_address = "0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52".to_string();

    let request = SearchSourcifySourcesRequest {
        chain_id,
        contract_address,
    };

    let response = reqwest::Client::new()
        .post(eth_bytecode_db_base.join(ROUTE).unwrap())
        .json(&request)
        .send()
        .await
        .expect("Failed to send request");

    // Assert that status code is success
    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await.expect("Read body as text");
        panic!("Invalid status code (success expected). Status: {status}. Message: {message}")
    }

    let verification_response: SearchSourcesResponse = response
        .json()
        .await
        .expect("Response deserialization failed");

    let expected_sources: Vec<Source> = vec![
        Source {
            file_name: "contracts/project:/ExternalTestMultiple.sol".to_string(),
            contract_name: "ExternalTestMultiple".to_string(),
            compiler_version: "v0.6.8+commit.0bbfe453".to_string(),
            compiler_settings: "{\r\n        \"evmVersion\": \"istanbul\",\r\n        \"libraries\": {},\r\n        \"metadata\": {\r\n            \"bytecodeHash\": \"ipfs\"\r\n        },\r\n        \"optimizer\": {\r\n            \"enabled\": true,\r\n            \"runs\": 300\r\n        },\r\n        \"remappings\": []\r\n    }".to_string(),
            source_type: eth_bytecode_db_v2::source::SourceType::Solidity.into(),
            source_files: BTreeMap::from([("contracts/project:/ExternalTestMultiple.sol".to_string(), "//SPDX-License-Identifier: MIT\r\npragma solidity ^0.6.8;\r\n\r\nlibrary ExternalTestLibraryMultiple {\r\n  function pop(address[] storage list) external returns (address out) {\r\n    out = list[list.length - 1];\r\n    list.pop();\r\n  }\r\n}\r\n".to_string())]),
            abi: Some("[\r\n    {\r\n        \"anonymous\": false,\r\n        \"inputs\": [],\r\n        \"name\": \"SourcifySolidity14\",\r\n        \"type\": \"event\"\r\n    },\r\n    {\r\n        \"inputs\": [\r\n            {\r\n                \"internalType\": \"address\",\r\n                \"name\": \"input\",\r\n                \"type\": \"address\"\r\n            }\r\n        ],\r\n        \"name\": \"identity\",\r\n        \"outputs\": [\r\n            {\r\n                \"internalType\": \"address\",\r\n                \"name\": \"\",\r\n                \"type\": \"address\"\r\n            }\r\n        ],\r\n        \"stateMutability\": \"nonpayable\",\r\n        \"type\": \"function\"\r\n    }\r\n]".to_string()),
            constructor_arguments: None,
            match_type: eth_bytecode_db_v2::source::MatchType::Full.into(),
        }
    ];

    assert_eq!(
        expected_sources, verification_response.sources,
        "Invalid sources returned"
    );
}
