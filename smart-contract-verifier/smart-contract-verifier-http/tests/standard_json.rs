mod standard_json_types;

use actix_web::{
    dev::ServiceResponse,
    test::{self, read_body, read_body_json, TestRequest},
    App,
};
use ethers_solc::artifacts::StandardJsonCompilerInput;
use pretty_assertions::assert_eq;
use serde_json::json;
use smart_contract_verifier_http::{
    configure_router, AppRouter, DisplayBytes, Settings, VerificationResponse, VerificationStatus,
};
use standard_json_types::TestInput;
use std::{
    collections::BTreeMap,
    fs,
    str::{from_utf8, FromStr},
};
use tokio::sync::OnceCell;

const CONTRACTS_DIR: &str = "tests/contracts";
const ROUTE: &str = "/api/v1/solidity/verify/standard-json";

async fn global_app_router() -> &'static AppRouter {
    static APP_ROUTER: OnceCell<AppRouter> = OnceCell::const_new();
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

async fn test_setup(dir: &str, input: &mut TestInput) -> (ServiceResponse, Option<DisplayBytes>) {
    let app_router = global_app_router().await;
    let app = test::init_service(App::new().configure(configure_router(app_router))).await;

    let prefix = format!("{CONTRACTS_DIR}/{dir}");
    let contract_path = format!("{prefix}/standard_input.json");
    input.standard_input = Some(input.standard_input.clone().unwrap_or_else(|| {
        fs::read_to_string(&contract_path).expect("Error while reading source")
    }));
    input.creation_tx_input = if !input.ignore_creation_tx_input {
        Some(input.creation_tx_input.clone().unwrap_or_else(|| {
            fs::read_to_string(format!("{prefix}/creation_tx_input"))
                .expect("Error while reading creation_tx_input")
        }))
    } else {
        None
    };
    input.deployed_bytecode = Some(input.deployed_bytecode.clone().unwrap_or_else(|| {
        fs::read_to_string(format!("{prefix}/deployed_bytecode"))
            .expect("Error while reading deployed_bytecode")
    }));
    let expected_constructor_argument = input.has_constructor_args.then(|| {
        DisplayBytes::from_str(
            &fs::read_to_string(format!("{prefix}/constructor_arguments"))
                .expect("Error while reading constructor_arguments"),
        )
        .expect("Expected constructor args must be valid")
    });

    let request = json!({
        "deployed_bytecode": input.deployed_bytecode.as_ref().unwrap(),
        "creation_bytecode": input.creation_tx_input.as_ref(),
        "compiler_version": input.compiler_version,
        "input": input.standard_input
    });

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
        panic!("Invalid status code (success expected). Status: {status}. Messsage: {message}")
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

    let abi: Option<Result<ethabi::Contract, _>> = verification_result
        .abi
        .as_ref()
        .map(|abi| serde_json::from_str(abi));
    assert_eq!(
        verification_result.contract_name, input.contract_name,
        "Invalid contract name"
    );
    if !input.is_yul {
        assert!(abi.is_some(), "Solidity contracts must have abi");
        assert!(
            abi.as_ref().unwrap().is_ok(),
            "Abi deserialization failed: {}",
            abi.unwrap().as_ref().unwrap_err()
        );
    }
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
            .for_each(|(_path, libs)| {
                libs.into_iter().for_each(|(contract, address)| {
                    formatted_libs.insert(contract, address);
                })
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
    let sources: BTreeMap<_, _> = standard_input
        .sources
        .into_iter()
        .map(|(path, source)| {
            (
                path.to_str().unwrap().to_string(),
                (*source.content).clone(),
            )
        })
        .collect();
    assert_eq!(verification_result.sources, sources, "Invalid source");
}

mod success_tests {
    use super::*;

    // Compilers from 0.4.11 to 0.4.18 have output everything regardless of
    // what was specified in outputSelection. That test checks that even in that
    // case resultant output could be parsed successfully.
    #[actix_rt::test]
    async fn solidity_0_4_11_to_0_4_18() {
        let contract_dir = "solidity_0.4.18";
        let test_input = TestInput::new("Main", "v0.4.18+commit.9cf6e910");
        test_success(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    async fn yul() {
        let contract_dir = "yul";
        let test_input = TestInput::new("Proxy", "v0.8.7+commit.e28d00a7").set_is_yul();
        test_success(contract_dir, test_input).await
    }
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

    #[actix_rt::test]
    async fn issue_with_creation_code() {
        let contract_dir = "issue_with_creation_code";
        let test_input =
            TestInput::new("PancakeFactory", "v0.5.16+commit.9c3226ce").has_constructor_args();
        test_success(contract_dir, test_input).await;
    }

    #[actix_rt::test]
    async fn issue_6275() {
        let contract_dir = "issue_6275";
        let test_input =
            TestInput::new("MultichainProxy", "v0.8.16+commit.07a7930e").ignore_creation_tx_input();
        test_success(contract_dir, test_input).await;
    }
}
