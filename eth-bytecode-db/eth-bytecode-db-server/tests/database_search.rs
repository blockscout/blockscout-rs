mod verification_test_helpers;

use crate::verification_test_helpers::{
    init_db, init_eth_bytecode_db_server, init_verifier_server,
};
use async_trait::async_trait;
use eth_bytecode_db::{verification, verification::MatchType};
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::{
    v2 as eth_bytecode_db_v2,
    v2::{BytecodeType, SearchSourcesRequest, SearchSourcesResponse, Source},
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

const ROUTE: &str = "/api/v2/bytecodes/sources:search";

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
async fn search_in_sourcify(service: MockSolidityVerifierService) {
    let db = init_db(TEST_SUITE_NAME, "test_returns_valid_source").await;

    let test_data = test_input_data::basic(verification::SourceType::Solidity, MatchType::Partial);

    let db_url = db.db_url();
    let verifier_addr = init_verifier_server(service, test_data.verifier_response).await;

    let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

    let bytecode = "0x608060405234801561001057600080fd5b506101ac806100206000396000f3fe608060405234801561001057600080fd5b506004361061002b5760003560e01c8063f0eb5e5414610030575b600080fd5b6100566004803603602081101561004657600080fd5b50356001600160a01b0316610072565b604080516001600160a01b039092168252519081900360200190f35b6040516000907fcd6e305ffe05775ee4dccd218c885635a575631eb3fe360b322621bad158facb908290a1600080546001810182558180527f290decd9548b62a8d60345a988386fc84ba6bc95484008f6362f93160ef3e56301805473ffffffffffffffffffffffffffffffffffffffff19166001600160a01b038516179055604080516374f0fffb60e01b8152600481019290925251736b88c55cfbd4eda1320f802b724193cab062ccce916374f0fffb916024808301926020929190829003018186803b15801561014457600080fd5b505af4158015610158573d6000803e3d6000fd5b505050506040513d602081101561016e57600080fd5b50519291505056fea26469706673582212205d1888f7386285c3a4057473423de59284f625b9678dc83756b94cdba366949d64736f6c63430006080033";
    let request = SearchSourcesRequest {
        bytecode: bytecode.to_string(),
        bytecode_type: BytecodeType::CreationInput.into(),
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
            compiler_settings: "{\r\n    \"compilationTarget\": {\r\n        \"contracts/project:/ExternalTestMultiple.sol\": \"ExternalTestMultiple\"\r\n    },\r\n    \"evmVersion\": \"istanbul\",\r\n    \"libraries\": {},\r\n    \"metadata\": {\r\n        \"bytecodeHash\": \"ipfs\"\r\n    },\r\n    \"optimizer\": {\r\n        \"enabled\": true,\r\n        \"runs\": 300\r\n    },\r\n    \"remappings\": []\r\n}".to_string(),
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
