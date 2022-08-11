mod standard_json_types;

use actix_web::{
    dev::ServiceResponse,
    test::{self, read_body, read_body_json, TestRequest},
    App,
};
use async_once_cell::OnceCell;
use ethers_solc::artifacts::StandardJsonCompilerInput;
use pretty_assertions::assert_eq;
use serde_json::json;
use standard_json_types::TestInput;
use std::collections::BTreeMap;
use std::{
    fs,
    str::{from_utf8, FromStr},
};
use verification::{
    configure_router, AppRouter, Config, DisplayBytes, VerificationResponse, VerificationStatus,
};

const CONTRACTS_DIR: &'static str = "tests/contracts";
const ROUTE: &'static str = "/api/v1/solidity/verify/standard-json";

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

async fn test_setup(dir: &str, input: &mut TestInput) -> (ServiceResponse, Option<DisplayBytes>) {
    let app_router = global_app_router().await;
    let app = test::init_service(App::new().configure(configure_router(app_router))).await;

    let prefix = format!("{}/{}", CONTRACTS_DIR, dir);
    let contract_path = format!("{}/standard_input.json", prefix);
    input.standard_input = Some(input.standard_input.clone().unwrap_or_else(|| {
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

    let request = json!({
        "deployed_bytecode": input.deployed_bytecode.as_ref().unwrap(),
        "creation_bytecode": input.creation_tx_input.as_ref().unwrap(),
        "compiler_version": input.compiler_version,
        "input": input.standard_input
    });

    // let request = if let Some(optimization_runs) = input.optimization_runs {
    //     json!({
    //         "deployed_bytecode": input.deployed_bytecode.as_ref().unwrap(),
    //         "creation_bytecode": input.creation_tx_input.as_ref().unwrap(),
    //         "compiler_version": input.compiler_version,
    //         "sources": BTreeMap::from([(contract_path, input.source_code.as_ref().unwrap())]),
    //         "evm_version": input.evm_version,
    //         "contract_libraries": input.contract_libraries,
    //         "optimization_runs": optimization_runs
    //     })
    // } else {
    //     json!({
    //         "deployed_bytecode": input.deployed_bytecode.as_ref().unwrap(),
    //         "creation_bytecode": input.creation_tx_input.as_ref().unwrap(),
    //         "compiler_version": input.compiler_version,
    //         "sources": BTreeMap::from([(contract_path, input.source_code.as_ref().unwrap())]),
    //         "evm_version": input.evm_version,
    //         "contract_libraries": input.contract_libraries
    //     })
    // };

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

    let verification_result = verification_response
        .result
        .expect("Verification result is not Some");

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

    let standard_input: StandardJsonCompilerInput =
        serde_json::from_str(&input.standard_input.expect("Set `Some` on test_setup"))
            .expect("Standard input deserialization");

    assert_eq!(
        verification_result.evm_version,
        standard_input
            .settings
            .evm_version
            .map(|version| version.to_string())
            .unwrap_or_else(|| "default".to_string()),
        "Invalid evm version"
    );
    assert_eq!(
        verification_result.compiler_version, input.compiler_version,
        "Invalid compiler version"
    );
    let libs = {
        let mut formatted_libs = BTreeMap::new();
        standard_input
            .settings
            .libraries
            .libs
            .into_iter()
            .for_each(|(path, libs)| {
                libs.into_iter()
                    .for_each(|(contract, address)| { formatted_libs.insert(format!("{}:{}", path.to_str().unwrap(), contract), address); })
            });
        formatted_libs
    };
    assert_eq!(
        verification_result.contract_libraries, libs,
        "Invalid contract libraries"
    );
    assert_eq!(
        verification_result.optimization, standard_input.settings.optimizer.enabled,
        "Invalid optimization"
    );
    assert_eq!(
        verification_result.optimization_runs, standard_input.settings.optimizer.runs,
        "Invalid optimization runs"
    );
    assert_eq!(
        verification_result.sources.len(),
        standard_input.sources.len(),
        "Invalid number of sources"
    );
    // let a = standard_input.sources.into_iter().map(|(path, source)| path.as_str)
    // assert_eq!(
    //     verification_result.sources.into_iter().values().next().unwrap(),
    //     &standard_input.sources.expect("Set `Some` on test_setup"),
    //     "Invalid source"
    // );
}

mod regression_tests {
    use super::*;

    // https://github.com/blockscout/blockscout/issues/5748
    #[actix_rt::test]
    async fn issue_5748() {
        let contract_dir = "issue_5748";
        let test_input = TestInput::new("ExternalTestJson", "v0.6.8+commit.0bbfe453");
        test_success(contract_dir, test_input).await;
    }
}
