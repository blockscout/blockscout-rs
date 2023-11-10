mod verification_test_helpers;

use crate::verification_test_helpers::{
    init_db, init_eth_bytecode_db_server, init_verifier_server,
};
use async_trait::async_trait;
use blockscout_service_launcher::test_server;
use eth_bytecode_db::{verification, verification::MatchType};
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::{
    v2 as eth_bytecode_db_v2,
    v2::{
        SearchAllSourcesRequest, SearchAllSourcesResponse, SearchSourcesResponse,
        SearchSourcifySourcesRequest, Source,
    },
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

const VERIFY_ROUTE: &str = "/api/v2/verifier/solidity/sources:verify-multi-part";

fn default_verify_request() -> eth_bytecode_db_v2::VerifySolidityMultiPartRequest {
    eth_bytecode_db_v2::VerifySolidityMultiPartRequest {
        bytecode: "".to_string(),
        bytecode_type: eth_bytecode_db_v2::BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        evm_version: None,
        optimization_runs: None,
        source_files: Default::default(),
        libraries: Default::default(),
        metadata: None,
    }
}

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

    let db = init_db(TEST_SUITE_NAME, "search_sourcify_sources").await;

    let test_data = test_input_data::basic(verification::SourceType::Solidity, MatchType::Partial);

    let db_url = db.db_url();
    let verifier_addr = init_verifier_server(service, test_data.verifier_response).await;

    let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

    let chain_id = "5".to_string();
    let contract_address = "0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52".to_string();

    let request = SearchSourcifySourcesRequest {
        chain: chain_id,
        address: contract_address,
    };

    let verification_response: SearchSourcesResponse =
        test_server::send_post_request(&eth_bytecode_db_base, ROUTE, &request).await;

    let expected_sources: Vec<Source> = vec![
        Source {
            file_name: "contracts/project:/ExternalTestMultiple.sol".to_string(),
            contract_name: "ExternalTestMultiple".to_string(),
            compiler_version: "0.6.8+commit.0bbfe453".to_string(),
            compiler_settings: "{\"evmVersion\":\"istanbul\",\"libraries\":{},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"optimizer\":{\"enabled\":true,\"runs\":300},\"remappings\":[]}".to_string(),
            source_type: eth_bytecode_db_v2::source::SourceType::Solidity.into(),
            source_files: BTreeMap::from([("contracts/project_/ExternalTestMultiple.sol".to_string(), "//SPDX-License-Identifier: MIT\r\npragma solidity ^0.6.8;\r\n\r\nlibrary ExternalTestLibraryMultiple {\r\n  function pop(address[] storage list) external returns (address out) {\r\n    out = list[list.length - 1];\r\n    list.pop();\r\n  }\r\n}\r\n".to_string())]),
            abi: Some("[{\"anonymous\":false,\"inputs\":[],\"name\":\"SourcifySolidity14\",\"type\":\"event\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"input\",\"type\":\"address\"}],\"name\":\"identity\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"\",\"type\":\"address\"}],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}]".to_string()),
            constructor_arguments: None,
            match_type: eth_bytecode_db_v2::source::MatchType::Full.into(),
            compilation_artifacts: None,
            creation_input_artifacts: None,
            deployed_bytecode_artifacts: None,
        }
    ];

    assert_eq!(
        expected_sources, verification_response.sources,
        "Invalid sources returned"
    );
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn search_all_sources(service: MockSolidityVerifierService) {
    const ROUTE: &str = "/api/v2/bytecodes/sources:search-all";

    let db = init_db(TEST_SUITE_NAME, "search_all_sources").await;

    let test_data = {
        let extra_data = smart_contract_verifier_v2::verify_response::ExtraData {
            local_creation_input_parts: vec![
                smart_contract_verifier_v2::verify_response::extra_data::BytecodePart {
                    r#type: "main".to_string(),
                    data: "0x608060405234801561001057600080fd5b506101ac806100206000396000f3fe608060405234801561001057600080fd5b506004361061002b5760003560e01c8063f0eb5e5414610030575b600080fd5b6100566004803603602081101561004657600080fd5b50356001600160a01b0316610072565b604080516001600160a01b039092168252519081900360200190f35b6040516000907fcd6e305ffe05775ee4dccd218c885635a575631eb3fe360b322621bad158facb908290a1600080546001810182558180527f290decd9548b62a8d60345a988386fc84ba6bc95484008f6362f93160ef3e56301805473ffffffffffffffffffffffffffffffffffffffff19166001600160a01b038516179055604080516374f0fffb60e01b8152600481019290925251736b88c55cfbd4eda1320f802b724193cab062ccce916374f0fffb916024808301926020929190829003018186803b15801561014457600080fd5b505af4158015610158573d6000803e3d6000fd5b505050506040513d602081101561016e57600080fd5b50519291505056fe".to_string(),
                },
                smart_contract_verifier_v2::verify_response::extra_data::BytecodePart {
                    r#type: "meta".to_string(),
                    data: "0xa264697066735822cafecafecafecafecafecafecafecafecafecafecafecafecafecafecafecafecafe64736f6c63430006080033".to_string(),
                },
            ],
            local_deployed_bytecode_parts: vec![
                smart_contract_verifier_v2::verify_response::extra_data::BytecodePart {
                    r#type: "main".to_string(),
                    data: "0x608060405234801561001057600080fd5b506004361061002b5760003560e01c8063f0eb5e5414610030575b600080fd5b6100566004803603602081101561004657600080fd5b50356001600160a01b0316610072565b604080516001600160a01b039092168252519081900360200190f35b6040516000907fcd6e305ffe05775ee4dccd218c885635a575631eb3fe360b322621bad158facb908290a1600080546001810182558180527f290decd9548b62a8d60345a988386fc84ba6bc95484008f6362f93160ef3e56301805473ffffffffffffffffffffffffffffffffffffffff19166001600160a01b038516179055604080516374f0fffb60e01b8152600481019290925251736b88c55cfbd4eda1320f802b724193cab062ccce916374f0fffb916024808301926020929190829003018186803b15801561014457600080fd5b505af4158015610158573d6000803e3d6000fd5b505050506040513d602081101561016e57600080fd5b50519291505056fe".to_string(),
                },
                smart_contract_verifier_v2::verify_response::extra_data::BytecodePart {
                    r#type: "meta".to_string(),
                    data: "0xa264697066735822cafecafecafecafecafecafecafecafecafecafecafecafecafecafecafecafecafe64736f6c63430006080033".to_string(),
                },
            ],
        };

        let mut test_data =
            test_input_data::basic(verification::SourceType::Solidity, MatchType::Partial);
        test_data.set_bytecode(extra_data);

        test_data
    };

    let db_url = db.db_url();
    let verifier_addr = init_verifier_server(service, test_data.verifier_response).await;

    let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

    // Fill the database with existing value
    {
        let dummy_request = default_verify_request();
        let _verification_response: eth_bytecode_db_v2::VerifyResponse =
            test_server::send_post_request(&eth_bytecode_db_base, VERIFY_ROUTE, &dummy_request)
                .await;
    }

    let chain_id = "5".to_string();
    let contract_address = "0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52".to_string();

    let request = SearchAllSourcesRequest {
        bytecode: "0x608060405234801561001057600080fd5b506101ac806100206000396000f3fe608060405234801561001057600080fd5b506004361061002b5760003560e01c8063f0eb5e5414610030575b600080fd5b6100566004803603602081101561004657600080fd5b50356001600160a01b0316610072565b604080516001600160a01b039092168252519081900360200190f35b6040516000907fcd6e305ffe05775ee4dccd218c885635a575631eb3fe360b322621bad158facb908290a1600080546001810182558180527f290decd9548b62a8d60345a988386fc84ba6bc95484008f6362f93160ef3e56301805473ffffffffffffffffffffffffffffffffffffffff19166001600160a01b038516179055604080516374f0fffb60e01b8152600481019290925251736b88c55cfbd4eda1320f802b724193cab062ccce916374f0fffb916024808301926020929190829003018186803b15801561014457600080fd5b505af4158015610158573d6000803e3d6000fd5b505050506040513d602081101561016e57600080fd5b50519291505056fea26469706673582212205d1888f7386285c3a4057473423de59284f625b9678dc83756b94cdba366949d64736f6c63430006080033".to_string(),
        bytecode_type: eth_bytecode_db_v2::BytecodeType::CreationInput.into(),
        chain: chain_id,
        address: contract_address,
    };

    let verification_response: SearchAllSourcesResponse =
        test_server::send_post_request(&eth_bytecode_db_base, ROUTE, &request).await;

    let expected_response = SearchAllSourcesResponse {
      eth_bytecode_db_sources: vec![
              test_data.eth_bytecode_db_response.source.unwrap()
          ],
        sourcify_sources: vec![
            Source {
                file_name: "contracts/project:/ExternalTestMultiple.sol".to_string(),
                contract_name: "ExternalTestMultiple".to_string(),
                compiler_version: "0.6.8+commit.0bbfe453".to_string(),
                compiler_settings: "{\"evmVersion\":\"istanbul\",\"libraries\":{},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"optimizer\":{\"enabled\":true,\"runs\":300},\"remappings\":[]}".to_string(),
                source_type: eth_bytecode_db_v2::source::SourceType::Solidity.into(),
                source_files: BTreeMap::from([("contracts/project_/ExternalTestMultiple.sol".to_string(), "//SPDX-License-Identifier: MIT\r\npragma solidity ^0.6.8;\r\n\r\nlibrary ExternalTestLibraryMultiple {\r\n  function pop(address[] storage list) external returns (address out) {\r\n    out = list[list.length - 1];\r\n    list.pop();\r\n  }\r\n}\r\n".to_string())]),
                abi: Some("[{\"anonymous\":false,\"inputs\":[],\"name\":\"SourcifySolidity14\",\"type\":\"event\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"input\",\"type\":\"address\"}],\"name\":\"identity\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"\",\"type\":\"address\"}],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}]".to_string()),
                constructor_arguments: None,
                match_type: eth_bytecode_db_v2::source::MatchType::Full.into(),
                compilation_artifacts: None,
                creation_input_artifacts: None,
                deployed_bytecode_artifacts: None,
            }
        ]
    };

    assert_eq!(
        expected_response, verification_response,
        "Invalid response returned"
    );
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn search_sources_returns_latest_contract() {
    const ROUTE: &str = "/api/v2/bytecodes/sources:search";

    let db = init_db(TEST_SUITE_NAME, "search_sources_returns_latest_contract").await;

    let build_test_data = |metadata_hash: &str| {
        let extra_data = smart_contract_verifier_v2::verify_response::ExtraData {
            local_creation_input_parts: vec![
                smart_contract_verifier_v2::verify_response::extra_data::BytecodePart {
                    r#type: "main".to_string(),
                    data: "0x608060405234801561001057600080fd5b506101ac806100206000396000f3fe608060405234801561001057600080fd5b506004361061002b5760003560e01c8063f0eb5e5414610030575b600080fd5b6100566004803603602081101561004657600080fd5b50356001600160a01b0316610072565b604080516001600160a01b039092168252519081900360200190f35b6040516000907fcd6e305ffe05775ee4dccd218c885635a575631eb3fe360b322621bad158facb908290a1600080546001810182558180527f290decd9548b62a8d60345a988386fc84ba6bc95484008f6362f93160ef3e56301805473ffffffffffffffffffffffffffffffffffffffff19166001600160a01b038516179055604080516374f0fffb60e01b8152600481019290925251736b88c55cfbd4eda1320f802b724193cab062ccce916374f0fffb916024808301926020929190829003018186803b15801561014457600080fd5b505af4158015610158573d6000803e3d6000fd5b505050506040513d602081101561016e57600080fd5b50519291505056fe".to_string(),
                },
                smart_contract_verifier_v2::verify_response::extra_data::BytecodePart {
                    r#type: "meta".to_string(),
                    data: format!("0xa264697066735822{metadata_hash}64736f6c63430006080033"),
                },
            ],
            local_deployed_bytecode_parts: vec![
                smart_contract_verifier_v2::verify_response::extra_data::BytecodePart {
                    r#type: "main".to_string(),
                    data: "0x608060405234801561001057600080fd5b506004361061002b5760003560e01c8063f0eb5e5414610030575b600080fd5b6100566004803603602081101561004657600080fd5b50356001600160a01b0316610072565b604080516001600160a01b039092168252519081900360200190f35b6040516000907fcd6e305ffe05775ee4dccd218c885635a575631eb3fe360b322621bad158facb908290a1600080546001810182558180527f290decd9548b62a8d60345a988386fc84ba6bc95484008f6362f93160ef3e56301805473ffffffffffffffffffffffffffffffffffffffff19166001600160a01b038516179055604080516374f0fffb60e01b8152600481019290925251736b88c55cfbd4eda1320f802b724193cab062ccce916374f0fffb916024808301926020929190829003018186803b15801561014457600080fd5b505af4158015610158573d6000803e3d6000fd5b505050506040513d602081101561016e57600080fd5b50519291505056fe".to_string(),
                },
                smart_contract_verifier_v2::verify_response::extra_data::BytecodePart {
                    r#type: "meta".to_string(),
                    data: format!("0xa264697066735822{metadata_hash}64736f6c63430006080033"),
                },
            ],
        };

        let mut test_data =
            test_input_data::basic(verification::SourceType::Solidity, MatchType::Partial);
        test_data.set_bytecode(extra_data);

        test_data
    };

    let test_data_old =
        build_test_data("cafecafecafecafecafecafecafecafecafecafecafecafecafecafecafecafecafe");
    {
        let db_url = db.db_url();
        let verifier_addr = init_verifier_server(service(), test_data_old.verifier_response).await;

        let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

        // Fill the database with existing value
        {
            let dummy_request = default_verify_request();
            let _verification_response: eth_bytecode_db_v2::VerifyResponse =
                test_server::send_post_request(&eth_bytecode_db_base, VERIFY_ROUTE, &dummy_request)
                    .await;
        }
    }

    let test_data_new = {
        let mut test_data =
            build_test_data("12341234123412341234123412341234123412341234123412341234123412341234");
        test_data.add_source_file(
            "Additional.sol".to_string(),
            "AdditionalContent".to_string(),
        );
        test_data
    };

    let db_url = db.db_url();
    let verifier_addr = init_verifier_server(service(), test_data_new.verifier_response).await;

    let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

    // Fill the database with existing value
    {
        let dummy_request = default_verify_request();
        let _verification_response: eth_bytecode_db_v2::VerifyResponse =
            test_server::send_post_request(&eth_bytecode_db_base, VERIFY_ROUTE, &dummy_request)
                .await;
    }

    let chain_id = "5".to_string();
    let contract_address = "0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52".to_string();

    let request = SearchAllSourcesRequest {
        bytecode: "0x608060405234801561001057600080fd5b506101ac806100206000396000f3fe608060405234801561001057600080fd5b506004361061002b5760003560e01c8063f0eb5e5414610030575b600080fd5b6100566004803603602081101561004657600080fd5b50356001600160a01b0316610072565b604080516001600160a01b039092168252519081900360200190f35b6040516000907fcd6e305ffe05775ee4dccd218c885635a575631eb3fe360b322621bad158facb908290a1600080546001810182558180527f290decd9548b62a8d60345a988386fc84ba6bc95484008f6362f93160ef3e56301805473ffffffffffffffffffffffffffffffffffffffff19166001600160a01b038516179055604080516374f0fffb60e01b8152600481019290925251736b88c55cfbd4eda1320f802b724193cab062ccce916374f0fffb916024808301926020929190829003018186803b15801561014457600080fd5b505af4158015610158573d6000803e3d6000fd5b505050506040513d602081101561016e57600080fd5b50519291505056fea26469706673582212205d1888f7386285c3a4057473423de59284f625b9678dc83756b94cdba366949d64736f6c63430006080033".to_string(),
        bytecode_type: eth_bytecode_db_v2::BytecodeType::CreationInput.into(),
        chain: chain_id,
        address: contract_address,
    };

    let verification_response: SearchSourcesResponse =
        test_server::send_post_request(&eth_bytecode_db_base, ROUTE, &request).await;

    let expected_response = SearchSourcesResponse {
        sources: vec![
            test_data_new.eth_bytecode_db_response.source.unwrap(),
            test_data_old.eth_bytecode_db_response.source.unwrap(),
        ],
    };

    assert_eq!(
        expected_response, verification_response,
        "Invalid response returned"
    );
}
