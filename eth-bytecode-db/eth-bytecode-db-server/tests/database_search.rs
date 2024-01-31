mod verification_test_helpers;

use blockscout_display_bytes::Bytes as DisplayBytes;
use blockscout_service_launcher::test_server;
use eth_bytecode_db::{verification, verification::MatchType};
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::{
    v2 as eth_bytecode_db_v2,
    v2::{
        BatchSearchEventDescriptionsRequest, BatchSearchEventDescriptionsResponse,
        EventDescription, SearchAllSourcesRequest, SearchAllSourcesResponse,
        SearchAllianceSourcesRequest, SearchEventDescriptionsRequest,
        SearchEventDescriptionsResponse, SearchSourcesResponse, SearchSourcifySourcesRequest,
        Source,
    },
};
use pretty_assertions::assert_eq;
use rstest::{fixture, rstest};
use smart_contract_verifier_proto::{
    blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2,
    http_client::mock::MockSolidityVerifierService,
};
use std::{collections::BTreeMap, path::PathBuf, str::FromStr};
use verification_test_helpers::{
    init_db, init_eth_bytecode_db_server, init_verifier_server, test_input_data,
    verifier_alliance_setup, verifier_alliance_types,
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
    let verifier_addr = init_verifier_server::<
        _,
        eth_bytecode_db_v2::VerifySolidityMultiPartRequest,
        _,
    >(service, test_data.verifier_response)
    .await;

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
async fn search_all_sources(
    service: MockSolidityVerifierService,
    #[values(None, Some(false), Some(true))] only_local: Option<bool>,
    #[files("tests/alliance_test_cases/full_match.json")] test_case_path: PathBuf,
) {
    let test_name = format!("search_all_sources_{only_local:?}");
    const ROUTE: &str = "/api/v2/bytecodes/sources:search-all";

    let chain_id = "5";
    let contract_address = "0x027f1fe8BbC2a7E9fE97868E82c6Ec6939086c52";

    let test_case = {
        let mut test_case = verifier_alliance_types::TestCase::from_file(test_case_path);
        test_case.chain_id = usize::from_str(chain_id).unwrap();
        test_case.address = DisplayBytes::from_str(contract_address).unwrap();
        test_case
    };

    let verifier_alliance_setup::SetupData {
        eth_bytecode_db_base: eth_bytecode_db_with_alliance_base,
        eth_bytecode_db: db,
        test_case,
        ..
    } = verifier_alliance_setup::Setup::new(&test_name)
        .authorized()
        .setup_test_case(TEST_SUITE_NAME, test_case)
        .await;

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
    let verifier_addr = init_verifier_server::<
        _,
        eth_bytecode_db_v2::VerifySolidityMultiPartRequest,
        _,
    >(service, test_data.verifier_response)
    .await;

    let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

    // Fill the database with existing value
    {
        let dummy_request = default_verify_request();
        let _verification_response: eth_bytecode_db_v2::VerifyResponse =
            test_server::send_post_request(&eth_bytecode_db_base, VERIFY_ROUTE, &dummy_request)
                .await;
    }

    let request = SearchAllSourcesRequest {
        bytecode: "0x608060405234801561001057600080fd5b506101ac806100206000396000f3fe608060405234801561001057600080fd5b506004361061002b5760003560e01c8063f0eb5e5414610030575b600080fd5b6100566004803603602081101561004657600080fd5b50356001600160a01b0316610072565b604080516001600160a01b039092168252519081900360200190f35b6040516000907fcd6e305ffe05775ee4dccd218c885635a575631eb3fe360b322621bad158facb908290a1600080546001810182558180527f290decd9548b62a8d60345a988386fc84ba6bc95484008f6362f93160ef3e56301805473ffffffffffffffffffffffffffffffffffffffff19166001600160a01b038516179055604080516374f0fffb60e01b8152600481019290925251736b88c55cfbd4eda1320f802b724193cab062ccce916374f0fffb916024808301926020929190829003018186803b15801561014457600080fd5b505af4158015610158573d6000803e3d6000fd5b505050506040513d602081101561016e57600080fd5b50519291505056fea26469706673582212205d1888f7386285c3a4057473423de59284f625b9678dc83756b94cdba366949d64736f6c63430006080033".to_string(),
        bytecode_type: eth_bytecode_db_v2::BytecodeType::CreationInput.into(),
        chain: chain_id.to_string(),
        address: contract_address.to_string(),
        only_local,
    };

    let verification_response: SearchAllSourcesResponse =
        test_server::send_post_request(&eth_bytecode_db_with_alliance_base, ROUTE, &request).await;

    let expected_sourcify_sources = match only_local {
        Some(true) => vec![],
        None | Some(false) =>
            vec![
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

    let expected_response = SearchAllSourcesResponse {
        eth_bytecode_db_sources: vec![test_data.eth_bytecode_db_response.source.unwrap()],
        sourcify_sources: expected_sourcify_sources,
        alliance_sources: vec![test_case
            .to_test_input_data()
            .eth_bytecode_db_response
            .source
            .unwrap()],
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
async fn search_alliance_sources(
    #[files("tests/alliance_test_cases/*.json")] test_case_path: PathBuf,
) {
    const TEST_NAME: &str = "search_alliance_sources";
    const ROUTE: &str = "/api/v2/bytecodes/sources:search-alliance";

    let setup_data = verifier_alliance_setup::Setup::new(TEST_NAME)
        .authorized()
        .setup(TEST_SUITE_NAME, test_case_path)
        .await;

    let request = SearchAllianceSourcesRequest {
        chain: setup_data.test_case.chain_id.to_string(),
        address: setup_data.test_case.address.to_string(),
    };

    let verification_response: SearchSourcesResponse =
        test_server::send_post_request(&setup_data.eth_bytecode_db_base, ROUTE, &request).await;

    let expected_response = SearchSourcesResponse {
        sources: vec![setup_data
            .test_case
            .to_test_input_data()
            .eth_bytecode_db_response
            .source
            .unwrap()],
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
        let verifier_addr = init_verifier_server::<
            _,
            eth_bytecode_db_v2::VerifySolidityMultiPartRequest,
            _,
        >(service(), test_data_old.verifier_response)
        .await;

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
    let verifier_addr = init_verifier_server::<
        _,
        eth_bytecode_db_v2::VerifySolidityMultiPartRequest,
        _,
    >(service(), test_data_new.verifier_response)
    .await;

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
        only_local: None,
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

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn search_event_descriptions() {
    const ROUTE: &str = "/api/v2/event-descriptions:search";

    let db = init_db(TEST_SUITE_NAME, "search_event_descriptions").await;

    let abi = r#"[{"inputs":[{"internalType":"uint256","name":"val","type":"uint256"}],"stateMutability":"nonpayable","type":"constructor"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"uint256","name":"a","type":"uint256"}],"name":"A","type":"event"},{"anonymous":true,"inputs":[{"indexed":false,"internalType":"uint256","name":"start","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"middle","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"end","type":"uint256"}],"name":"Anonymous","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"string","name":"a","type":"string"},{"indexed":true,"internalType":"uint256","name":"b","type":"uint256"},{"indexed":true,"internalType":"uint256","name":"c","type":"uint256"},{"indexed":true,"internalType":"bytes","name":"d","type":"bytes"}],"name":"B","type":"event"},{"stateMutability":"payable","type":"fallback"},{"inputs":[],"name":"f","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"stateMutability":"payable","type":"receive"}]"#;

    let test_data = {
        let mut test_data =
            test_input_data::basic(verification::SourceType::Solidity, MatchType::Partial);
        test_data.set_abi(abi.to_string());
        test_data
    };

    let db_url = db.db_url();
    let verifier_addr = init_verifier_server::<
        _,
        eth_bytecode_db_v2::VerifySolidityMultiPartRequest,
        _,
    >(service(), test_data.verifier_response)
    .await;

    let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

    // Fill the database with existing value
    {
        let dummy_request = default_verify_request();
        let _verification_response: eth_bytecode_db_v2::VerifyResponse =
            test_server::send_post_request(&eth_bytecode_db_base, VERIFY_ROUTE, &dummy_request)
                .await;
    }

    let selector = "0xa17a9e66f0c355e3aa3b9ea969991204d6b1d2e62a47877f612cb2371d79e06a";

    let request = SearchEventDescriptionsRequest {
        selector: selector.into(),
    };

    let event_descriptions: SearchEventDescriptionsResponse =
        test_server::send_post_request(&eth_bytecode_db_base, ROUTE, &request).await;

    let expected_response = SearchEventDescriptionsResponse {
        event_descriptions: vec![EventDescription {
            r#type: "event".into(),
            name: "A".into(),
            inputs: r#"[{"indexed":true,"internalType":"uint256","name":"a","type":"uint256"}]"#
                .into(),
        }],
    };

    assert_eq!(
        expected_response, event_descriptions,
        "Invalid response returned"
    );
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn batch_search_event_descriptions() {
    const ROUTE: &str = "/api/v2/event-descriptions:batch-search";

    let db = init_db(TEST_SUITE_NAME, "batch_search_event_descriptions").await;

    let abi = r#"[{"inputs":[{"internalType":"uint256","name":"val","type":"uint256"}],"stateMutability":"nonpayable","type":"constructor"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"uint256","name":"a","type":"uint256"}],"name":"A","type":"event"},{"anonymous":true,"inputs":[{"indexed":false,"internalType":"uint256","name":"start","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"middle","type":"uint256"},{"indexed":false,"internalType":"uint256","name":"end","type":"uint256"}],"name":"Anonymous","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"internalType":"string","name":"a","type":"string"},{"indexed":true,"internalType":"uint256","name":"b","type":"uint256"},{"indexed":true,"internalType":"uint256","name":"c","type":"uint256"},{"indexed":true,"internalType":"bytes","name":"d","type":"bytes"}],"name":"B","type":"event"},{"stateMutability":"payable","type":"fallback"},{"inputs":[],"name":"f","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"},{"stateMutability":"payable","type":"receive"}]"#;

    let test_data = {
        let mut test_data =
            test_input_data::basic(verification::SourceType::Solidity, MatchType::Partial);
        test_data.set_abi(abi.to_string());
        test_data
    };

    let db_url = db.db_url();
    let verifier_addr = init_verifier_server::<
        _,
        eth_bytecode_db_v2::VerifySolidityMultiPartRequest,
        _,
    >(service(), test_data.verifier_response)
    .await;

    let eth_bytecode_db_base = init_eth_bytecode_db_server(db_url, verifier_addr).await;

    // Fill the database with existing value
    {
        let dummy_request = default_verify_request();
        let _verification_response: eth_bytecode_db_v2::VerifyResponse =
            test_server::send_post_request(&eth_bytecode_db_base, VERIFY_ROUTE, &dummy_request)
                .await;
    }

    let selectors = [
        "0xbcf5c814cb65249e306ec7aeaef6fc1ca35e1b8e18c08b054c9f9c76160bc923".to_string(),
        "0xa17a9e66f0c355e3aa3b9ea969991204d6b1d2e62a47877f612cb2371d79e06a".to_string(),
        "0x6bda65e31c7e349462fbf26f88a201b5f967d8582bcfe8d12b9be6ba824324a1".to_string(),
    ];

    let request = BatchSearchEventDescriptionsRequest {
        selectors: selectors.into(),
    };

    let batch_event_descriptions: BatchSearchEventDescriptionsResponse =
        test_server::send_post_request(&eth_bytecode_db_base, ROUTE, &request).await;

    let expected_response = BatchSearchEventDescriptionsResponse {
        responses: vec![
            SearchEventDescriptionsResponse {
                event_descriptions: vec![
                    EventDescription {
                        r#type: "event".into(),
                        name: "B".into(),
                        inputs: r#"[{"indexed":false,"internalType":"string","name":"a","type":"string"},{"indexed":true,"internalType":"uint256","name":"b","type":"uint256"},{"indexed":true,"internalType":"uint256","name":"c","type":"uint256"},{"indexed":true,"internalType":"bytes","name":"d","type":"bytes"}]"#.into(),
                    },
                ]
            },
            SearchEventDescriptionsResponse  {
                event_descriptions: vec![
                    EventDescription {
                        r#type: "event".into(),
                        name: "A".into(),
                        inputs: r#"[{"indexed":true,"internalType":"uint256","name":"a","type":"uint256"}]"#
                            .into(),
                    },
                ]
            },
            SearchEventDescriptionsResponse {
                event_descriptions: vec![]
            },
        ],
    };

    assert_eq!(
        expected_response, batch_event_descriptions,
        "Invalid response returned"
    );
}
