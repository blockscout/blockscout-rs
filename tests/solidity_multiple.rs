mod solidity_multiple_types;

use actix_web::{
    dev::ServiceResponse,
    test::{self, read_body, read_body_json, TestRequest},
    App,
};
use async_once_cell::OnceCell;
use serde_json::json;
use solidity_multiple_types::TestInput;
use std::{
    collections::BTreeMap,
    fs,
    str::{from_utf8, FromStr},
};
use verification::{
    configure_router, AppRouter, Config, DisplayBytes, VerificationResponse, VerificationStatus,
};

const CONTRACTS_DIR: &'static str = "tests/contracts";
const ROUTE: &'static str = "/api/v1/solidity/verify/multiple-files";

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

async fn test_setup(
    dir: &'static str,
    input: &mut TestInput,
) -> (ServiceResponse, Option<DisplayBytes>) {
    let app_router = global_app_router().await;
    let app = test::init_service(App::new().configure(configure_router(app_router))).await;

    let prefix = format!("{}/{}", CONTRACTS_DIR, dir);
    let contract_path = format!("{}/source.sol", prefix);
    input.source_code = Some(input.source_code.clone().unwrap_or_else(|| {
        fs::read_to_string(&contract_path).expect("Error while reading source")
    }));
    input.creation_tx_input = Some(input.creation_tx_input.clone().unwrap_or_else(|| {
        fs::read_to_string(format!("{}/creation_tx_input", prefix))
            .expect("Error while reading creation_tx_input")
    }));
    input.deployed_bytecode = Some(input.deployed_bytecode.clone().unwrap_or_else(|| {
        fs::read_to_string(format!("{}/deployed_bytecode", prefix))
            .expect("Error while reading deployed_bytecode")
    }));
    let expected_constructor_argument = input.has_constructor_args.then(|| {
        DisplayBytes::from_str(
            &fs::read_to_string(format!("{}/constructor_arguments", prefix))
                .expect("Error while reading constructor_arguments"),
        )
        .expect("Expected constructor args must be valid")
    });

    let request = if let Some(optimization_runs) = input.optimization_runs {
        json!({
            "deployed_bytecode": input.deployed_bytecode.as_ref().unwrap(),
            "creation_bytecode": input.creation_tx_input.as_ref().unwrap(),
            "compiler_version": input.compiler_version,
            "sources": BTreeMap::from([(contract_path, input.source_code.as_ref().unwrap())]),
            "evm_version": input.evm_version,
            "contract_libraries": input.contract_libraries,
            "optimization_runs": optimization_runs
        })
    } else {
        json!({
            "deployed_bytecode": input.deployed_bytecode.as_ref().unwrap(),
            "creation_bytecode": input.creation_tx_input.as_ref().unwrap(),
            "compiler_version": input.compiler_version,
            "sources": BTreeMap::from([(contract_path, input.source_code.as_ref().unwrap())]),
            "evm_version": input.evm_version,
            "contract_libraries": input.contract_libraries
        })
    };

    let response = TestRequest::post()
        .uri(ROUTE)
        .set_json(&request)
        .send_request(&app)
        .await;

    (response, expected_constructor_argument)
}

async fn test_success(dir: &'static str, mut input: TestInput) {
    let (response, expected_constructor_argument) = test_setup(dir, &mut input).await;

    // Assert that status code is success
    if !response.status().is_success() {
        let status = response.status();
        let body = read_body(response).await;
        let message = from_utf8(&body).expect("Read body as UTF-8");
        panic!(
            "Invalid status code (success expected). Status: {}. Messsage: {}",
            status, message
        )
    }

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
        &input.source_code.expect("Set `Some` on test_setup"),
        "Invalid source"
    );
}

/// Test verification failures (note: do not handle 400 BadRequest responses)
async fn test_failure<'a>(dir: &'static str, mut input: TestInput, expected_message: &'a str) {
    let (response, _expected_constructor_argument) = test_setup(dir, &mut input).await;

    assert!(
        response.status().is_success(),
        "Invalid status code (success expected): {}",
        response.status()
    );

    let verification_response: VerificationResponse = read_body_json(response).await;

    assert_eq!(
        verification_response.status,
        VerificationStatus::Failed,
        "Invalid verification status. Response: {:?}",
        verification_response
    );

    assert!(
        verification_response.result.is_none(),
        "Failure verification result should be None"
    );

    assert!(
        verification_response.message.contains(expected_message),
        "Invalid message: {}",
        verification_response.message
    );
}

mod success_tests {
    use super::*;

    #[actix_rt::test]
    async fn verifies_the_generated_bytecode_against_bytecode_retrieved_from_the_blockchain() {
        let contract_dir = "simple_storage";
        let test_input = TestInput::new("SimpleStorage", "v0.4.24+commit.e67f0147");
        test_success(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    async fn verifies_the_generated_bytecode_with_external_libraries() {
        let contract_dir = "contract_with_lib";
        let mut libraries = BTreeMap::new();
        libraries.insert(
            "BadSafeMath".to_string(),
            "0x9Bca1BF2810c9b68F25c82e8eBb9dC0A5301e310".to_string(),
        );
        // let test_input = TestInput::new("SimpleStorage", "v0.5.11+commit.c082d0b4")
        let test_input = TestInput::new("SimpleStorage", "v0.5.11+commit.22be8592")
            .with_optimization_runs(200)
            .with_contract_libraries(libraries);
        test_success(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    // `whisper` metadata - (bzz0 => bzz1) in solidity 0.5.11()
    async fn verifies_smart_contract_with_new_whisper_metadata() {
        let contract_dir = "solidity_5.11_new_whisper_metadata";
        let test_input = TestInput::new("FixedSupplyToken", "v0.5.11+commit.22be8592")
            .with_evm_version("byzantium");
        test_success(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    async fn verifies_library() {
        let contract_dir = "library";
        // let test_input = TestInput::new("Foo", ""v0.5.11+commit.c082d0b4"")
        let test_input =
            TestInput::new("Foo", "v0.5.11+commit.22be8592").with_optimization_runs(200);
        test_success(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    // (includes new metadata in bytecode)
    async fn verifies_smart_contract_compiled_with_solidity_0_5_9() {
        let contract_dir = "solidity_0.5.9_smart_contract";
        // let test_input = TestInput::new("TestToken", "v0.5.9+commit.e560f70d")
        let test_input = TestInput::new("TestToken", "v0.5.9+commit.c68bc34e")
            .with_evm_version("petersburg")
            .has_constructor_args();
        test_success(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    // https://solidity.readthedocs.io/en/v0.6.6/contracts.html?highlight=immutable#constant-and-immutable-state-variables
    async fn verifies_smart_contract_with_immutable_assignment() {
        let contract_dir = "with_immutable_assignment";
        let test_input = TestInput::new("C", "v0.6.7+commit.b8d736ae")
            .with_optimization_runs(200)
            .has_constructor_args();
        test_success(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    // verifies smart-contract created from another contract
    async fn contract_from_factory() {
        let contract_dir = "contract_from_factory";
        let test_input = TestInput::new("ContractFromFactory", "v0.4.26+commit.4563c3fc")
            .with_optimization_runs(200)
            .has_constructor_args();
        test_success(contract_dir, test_input).await;
    }
}

mod error_tests {
    use super::*;

    #[actix_rt::test]
    async fn returns_failure_when_bytecode_does_not_match() {
        let contract_dir = "simple_storage";
        let test_input = TestInput::new("SimpleStorage", "v0.4.24+commit.e67f0147")
            .with_source_code("pragma solidity ^0.4.24; contract SimpleStorage {}".to_string());
        test_failure(
            contract_dir,
            test_input,
            "No contract could be verified with provided data",
        )
        .await;
    }

    #[actix_rt::test]
    async fn returns_failure_with_compilation_problems() {
        let contract_dir = "simple_storage";
        let test_input = TestInput::new("SimpleStorage", "v0.4.24+commit.e67f0147")
            .with_source_code("pragma solidity ^0.4.24; contract SimpleStorage { ".to_string());
        test_failure(contract_dir, test_input, "ParserError").await;
    }
}

mod regression_tests {
    use super::*;

    // https://github.com/blockscout/blockscout/issues/5114
    #[actix_rt::test]
    async fn issue_5114() {
        let contract_dir = "issue_5114";
        let test_input = TestInput::new("TransparentUpgradeableProxy", "v0.8.2+commit.661d1103")
            .with_optimization_runs(200)
            .has_constructor_args();
        test_success(contract_dir, test_input).await;
    }

    // https://github.com/blockscout/blockscout/issues/5127
    #[actix_rt::test]
    async fn issue_5127() {
        let contract_dir = "issue_5127";
        let test_input = TestInput::new("YESToken", "v0.8.11+commit.d7f03943")
            .with_optimization_runs(200)
            .has_constructor_args();
        test_success(contract_dir, test_input).await;
    }

    // https://github.com/blockscout/blockscout/issues/3082
    #[actix_rt::test]
    async fn issue_3082() {
        let contract_dir = "issue_3082";
        let test_input = TestInput::new("Distribution", "v0.5.10+commit.5a6ea5b1")
            .with_optimization_runs(200)
            .has_constructor_args();
        test_success(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    async fn another_failed_constructor_args_verification() {
        let contract_dir = "issue_with_constructor_args";
        let test_input = TestInput::new("ERC1967Proxy", "v0.8.2+commit.661d1103")
            .with_optimization_runs(200)
            .has_constructor_args();
        test_success(contract_dir, test_input).await;
    }

    // https://github.com/blockscout/blockscout/issues/4758
    #[actix_rt::test]
    async fn issue_4758() {
        let contract_dir = "issue_4758";
        let test_input = TestInput::new("CS3_1OnChainShop", "v0.8.4+commit.c7e474f2")
            .with_optimization_runs(200)
            .has_constructor_args();
        test_success(contract_dir, test_input).await;
    }

    // runs only for linux, as other compiler lists do not have nightly builds
    #[cfg(target_os = "linux")]
    // https://github.com/blockscout/blockscout/issues/5430
    // https://github.com/blockscout/blockscout/issues/5434
    #[actix_rt::test]
    #[ignore] // remove when list with nightly builds would be ready for linux
    async fn issue_5430_5434() {
        let contract_dir = "issue_5430_5434";
        let test_input = TestInput::new("C", "v0.8.14-nightly.2022.4.13+commit.25923c1f");
        test_success(contract_dir, test_input).await;
    }

    // https://github.com/blockscout/blockscout/issues/5431
    #[actix_rt::test]
    // (smart contract was compiled with bytecodeHash=none; constructor with arguments)
    async fn issue_5431() {
        let contract_dir = "issue_5431";
        let test_input = TestInput::new("Owner", "v0.8.8+commit.dddeac2f").has_constructor_args();
        test_success(contract_dir, test_input).await;
    }

    // https://github.com/blockscout/blockscout/issues/5636
    // #[actix_rt::test]
    // // type(A).creationCode in the constructor
    // async fn issue_5636() {
    //     let contract_dir = "issue_5636";
    //     let test_input = TestInput::new("B", "v0.8.14+commit.80d49f37").with_optimization_runs(200);
    //     test(contract_dir, test_input).await;
    // }
}

mod tests_from_constructor_arguments_test_exs {
    use super::*;

    #[actix_rt::test]
    async fn verifies_with_require_messages() {
        let contract_dir = "home_bridge";
        let test_input = TestInput::new("HomeBridge", "v0.5.8+commit.23d335f2")
            .with_optimization_runs(200)
            .has_constructor_args();
        test_success(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    async fn verifies_with_string_in_keccak256() {
        let contract_dir = "ERC677";
        let test_input = TestInput::new("ERC677MultiBridgeToken", "v0.5.10+commit.5a6ea5b1")
            .with_optimization_runs(200)
            .has_constructor_args();
        test_success(contract_dir, test_input).await;
    }
}
