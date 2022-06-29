use actix_web::{
    test::{self, read_body_json, TestRequest},
    App,
};
use async_once_cell::OnceCell;
use serde_json::json;
use std::{collections::BTreeMap, fs, str::FromStr};
use verification::{
    configure_router, AppRouter, Config, DisplayBytes, VerificationResponse, VerificationStatus,
};

const CONTRACTS_DIR: &'static str = "tests/contracts";
const ROUTE: &'static str = "/api/v1/solidity/verify/multiple";

async fn global_app_router() -> &'static AppRouter {
    static APP_ROUTER: OnceCell<AppRouter> = OnceCell::new();
    APP_ROUTER
        .get_or_init(async {
            let mut config = Config::default();
            config.sourcify.enabled = false;
            AppRouter::new(config)
                .await
                .expect("couldn't initialize the app")
        })
        .await
}

struct TestInput {
    contract_name: &'static str,
    compiler_version: &'static str,
    evm_version: &'static str,
    optimization_runs: Option<usize>,
    contract_libraries: BTreeMap<String, String>,
    has_constructor_args: bool,
}

async fn test(dir: &'static str, input: TestInput) {
    let app_router = global_app_router().await;
    let app = test::init_service(App::new().configure(configure_router(app_router))).await;

    let prefix = format!("{}/{}", CONTRACTS_DIR, dir);
    let contract_path = format!("{}/{}.sol", prefix, dir);
    let source = fs::read_to_string(&contract_path).expect("Error while reading source");
    let creation_tx_input = fs::read_to_string(format!("{}/creation_tx_input", prefix))
        .expect("Error while reading creation_tx_input");
    let deployed_bytecode = fs::read_to_string(format!("{}/deployed_bytecode", prefix))
        .expect("Error while reading deployed_bytecode");
    let expected_constructor_argument = input.has_constructor_args.then(|| {
        DisplayBytes::from_str(
            &fs::read_to_string(format!("{}/constructor_arguments", prefix))
                .expect("Error while reading constructor_arguments"),
        )
        .expect("Expected constructor args must be valid")
    });

    let request = if let Some(optimization_runs) = input.optimization_runs {
        json!({
            "deployed_bytecode": deployed_bytecode,
            "creation_bytecode": creation_tx_input,
            "compiler_version": input.compiler_version,
            "sources": BTreeMap::from([(contract_path, source.clone())]),
            "evm_version": input.evm_version,
            "contract_libraries": input.contract_libraries,
            "optimization_runs": optimization_runs
        })
    } else {
        json!({
            "deployed_bytecode": deployed_bytecode,
            "creation_bytecode": creation_tx_input,
            "compiler_version": input.compiler_version,
            "sources": BTreeMap::from([(contract_path, source.clone())]),
            "evm_version": input.evm_version,
            "contract_libraries": input.contract_libraries
        })
    };

    let response = TestRequest::post()
        .uri(ROUTE)
        .set_json(&request)
        .send_request(&app)
        .await;

    assert!(
        response.status().is_success(),
        "Invalid status code (success expected): {}",
        response.status()
    );

    let verification_response: VerificationResponse = read_body_json(response).await;

    assert_eq!(
        verification_response.status,
        VerificationStatus::Ok,
        "Invalid verification status. Response: {:?}",
        verification_response
    );

    assert!(
        verification_response.result.is_some(),
        "Verification result is not Some"
    );

    let verification_result = verification_response.result.expect("Checked above");

    let abi: Result<ethabi::Contract, _> = serde_json::from_str(&verification_result.abi);
    assert_eq!(
        verification_result.contract_name, input.contract_name,
        "Invalid contract name"
    );
    assert!(
        abi.is_ok(),
        "Abi deserialization failed: {}",
        abi.unwrap_err()
    );
    assert_eq!(
        verification_result.constructor_arguments, expected_constructor_argument,
        "Invalid constructor args"
    );

    assert_eq!(
        verification_result.evm_version, input.evm_version,
        "Invalid evm version"
    );
    assert_eq!(
        verification_result.compiler_version, input.compiler_version,
        "Invalid compiler version"
    );
    assert_eq!(
        verification_result.contract_libraries, input.contract_libraries,
        "Invalid contract libraries"
    );
    assert_eq!(
        verification_result.optimization,
        Some(input.optimization_runs.is_some()),
        "Invalid optimization"
    );
    assert_eq!(
        verification_result.optimization_runs, input.optimization_runs,
        "Invalid optimization runs"
    );
    assert_eq!(
        verification_result.sources.len(),
        1,
        "Invalid number of sources"
    );
    assert_eq!(
        verification_result.sources.values().next().unwrap(),
        &source,
        "Invalid source"
    );
}

mod basic_tests {
    use super::*;

    #[actix_rt::test]
    async fn verifies_the_generated_bytecode_against_bytecode_retrieved_from_the_blockchain() {
        let contract_dir = "simple_storage";
        let test_input = TestInput {
            contract_name: "SimpleStorage",
            compiler_version: "v0.4.24+commit.e67f0147",
            evm_version: "default",
            optimization_runs: None,
            contract_libraries: Default::default(),
            has_constructor_args: false,
        };
        test(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    async fn verifies_the_generated_bytecode_with_external_libraries() {
        let contract_dir = "contract_with_lib";
        let mut libraries = BTreeMap::new();
        libraries.insert(
            "BadSafeMath".to_string(),
            "0x9Bca1BF2810c9b68F25c82e8eBb9dC0A5301e310".to_string(),
        );
        let test_input = TestInput {
            contract_name: "SimpleStorage",
            // compiler_version: "v0.5.11+commit.c082d0b4",
            compiler_version: "v0.5.11+commit.22be8592",
            evm_version: "default",
            optimization_runs: Some(200),
            contract_libraries: libraries,
            has_constructor_args: false,
        };
        test(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    // `whisper` metadata - (bzz0 => bzz1) in solidity 0.5.11()
    async fn verifies_smart_contract_with_new_whisper_metadata() {
        let contract_dir = "solidity_5.11_new_whisper_metadata";
        let test_input = TestInput {
            contract_name: "FixedSupplyToken",
            // compiler_version: "v0.5.11+commit.c082d0b4",
            compiler_version: "v0.5.11+commit.22be8592",
            evm_version: "byzantium",
            optimization_runs: None,
            contract_libraries: Default::default(),
            has_constructor_args: false,
        };
        test(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    async fn verifies_library() {
        let contract_dir = "library";
        let test_input = TestInput {
            contract_name: "Foo",
            // compiler_version: "v0.5.11+commit.c082d0b4",
            compiler_version: "v0.5.11+commit.22be8592",
            evm_version: "default",
            optimization_runs: Some(200),
            contract_libraries: Default::default(),
            has_constructor_args: false,
        };
        test(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    // (includes new metadata in bytecode)
    async fn verifies_smart_contract_compiled_with_solidity_0_5_9() {
        let contract_dir = "solidity_0.5.9_smart_contract";
        let test_input = TestInput {
            contract_name: "TestToken",
            // compiler_version: "v0.5.9+commit.e560f70d",
            compiler_version: "v0.5.9+commit.c68bc34e",
            evm_version: "petersburg",
            optimization_runs: None,
            contract_libraries: Default::default(),
            has_constructor_args: true,
        };
        test(contract_dir, test_input).await;
    }

    // #[actix_rt::test]
    // #[ignore]
    // // (includes new metadata in bytecode)
    // async fn returns_error_when_bytecode_does_not_match() {
    //     // let contract_dir = "solidity_0.5.9_smart_contract";
    //     // let test_input = TestInput {
    //     //     contract_name: "TestToken",
    //     //     compiler_version: "v0.5.9+commit.e560f70d",
    //     //     evm_version: "petersburg",
    //     //     optimization_runs: None,
    //     //     contract_libraries: Default::default(),
    //     //     has_constructor_args: true
    //     // };
    //     // test(contract_dir, test_input).await;
    // }

    #[actix_rt::test]
    // https://solidity.readthedocs.io/en/v0.6.6/contracts.html?highlight=immutable#constant-and-immutable-state-variables
    async fn verifies_smart_contract_with_immutable_assignment() {
        let contract_dir = "with_immutable_assignment";
        let test_input = TestInput {
            contract_name: "C",
            compiler_version: "v0.6.7+commit.b8d736ae",
            evm_version: "default",
            optimization_runs: Some(200),
            contract_libraries: Default::default(),
            has_constructor_args: true,
        };
        test(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    // verifies smart-contract created from another contract
    async fn contract_from_factory() {
        let contract_dir = "contract_from_factory";
        let test_input = TestInput {
            contract_name: "ContractFromFactory",
            compiler_version: "v0.4.26+commit.4563c3fc",
            evm_version: "default",
            optimization_runs: Some(200),
            contract_libraries: Default::default(),
            has_constructor_args: true,
        };
        test(contract_dir, test_input).await;
    }
}

mod regression_tests {
    use super::*;

    #[actix_rt::test]
    async fn issue_5114() {
        let contract_dir = "issue_5114";
        let test_input = TestInput {
            contract_name: "TransparentUpgradeableProxy",
            compiler_version: "v0.8.2+commit.661d1103",
            evm_version: "default",
            optimization_runs: Some(200),
            contract_libraries: Default::default(),
            has_constructor_args: true,
        };
        test(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    async fn issue_5127() {
        let contract_dir = "issue_5127";
        let test_input = TestInput {
            contract_name: "YESToken",
            compiler_version: "v0.8.11+commit.d7f03943",
            evm_version: "default",
            optimization_runs: Some(200),
            contract_libraries: Default::default(),
            has_constructor_args: true,
        };
        test(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    async fn issue_3082() {
        let contract_dir = "issue_3082";
        let test_input = TestInput {
            contract_name: "Distribution",
            compiler_version: "v0.5.10+commit.5a6ea5b1",
            evm_version: "default",
            optimization_runs: Some(200),
            contract_libraries: Default::default(),
            has_constructor_args: true,
        };
        test(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    async fn another_failed_constructor_args_verification() {
        let contract_dir = "issue_with_constructor_args";
        let test_input = TestInput {
            contract_name: "ERC1967Proxy",
            compiler_version: "v0.8.2+commit.661d1103",
            evm_version: "default",
            optimization_runs: Some(200),
            contract_libraries: Default::default(),
            has_constructor_args: true,
        };
        test(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    async fn issue_4758() {
        let contract_dir = "issue_4758";
        let test_input = TestInput {
            contract_name: "CS3_1OnChainShop",
            compiler_version: "v0.8.4+commit.c7e474f2",
            evm_version: "default",
            optimization_runs: Some(200),
            contract_libraries: Default::default(),
            has_constructor_args: true,
        };
        test(contract_dir, test_input).await;
    }

    // runs only for linux, as other compiler lists do not have nightly builds
    #[cfg(target_os = "linux")]
    #[actix_rt::test]
    #[ignore] // remove when list with nightly builds would be ready for linux
    async fn issue_5430_5434() {
        let contract_dir = "issue_5430_5434";
        let test_input = TestInput {
            contract_name: "C",
            compiler_version: "v0.8.14-nightly.2022.4.13+commit.25923c1f",
            evm_version: "default",
            optimization_runs: None,
            contract_libraries: Default::default(),
            has_constructor_args: false,
        };
        test(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    // (smart contract was compiled with bytecodeHash=none; constructor with arguments)
    async fn issue_5431() {
        let contract_dir = "issue_5431";
        let test_input = TestInput {
            contract_name: "Owner",
            compiler_version: "v0.8.8+commit.dddeac2f",
            evm_version: "default",
            optimization_runs: None,
            contract_libraries: Default::default(),
            has_constructor_args: true,
        };
        test(contract_dir, test_input).await;
    }

    // #[actix_rt::test]
    // // type(A).creationCode in the constructor
    // async fn issue_5636() {
    //     let contract_dir = "issue_5636";
    //     let test_input = TestInput {
    //         contract_name: "B",
    //         compiler_version: "v0.8.14+commit.80d49f37",
    //         evm_version: "default",
    //         optimization_runs: Some(200),
    //         contract_libraries: Default::default(),
    //         has_constructor_args: false,
    //     };
    //     test(contract_dir, test_input).await;
    // }
}

mod tests_from_constructor_arguments_test_exs {
    use super::*;

    #[actix_rt::test]
    async fn verifies_with_require_messages() {
        let contract_dir = "home_bridge";
        let test_input = TestInput {
            contract_name: "HomeBridge",
            compiler_version: "v0.5.8+commit.23d335f2",
            evm_version: "default",
            optimization_runs: Some(200),
            contract_libraries: Default::default(),
            has_constructor_args: true,
        };
        test(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    async fn verifies_with_string_in_keccak256() {
        let contract_dir = "ERC677";
        let test_input = TestInput {
            contract_name: "ERC677MultiBridgeToken",
            compiler_version: "v0.5.10+commit.5a6ea5b1",
            evm_version: "default",
            optimization_runs: Some(200),
            contract_libraries: Default::default(),
            has_constructor_args: true,
        };
        test(contract_dir, test_input).await;
    }
}
