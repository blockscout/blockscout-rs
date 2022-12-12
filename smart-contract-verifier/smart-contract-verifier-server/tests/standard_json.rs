mod standard_json_types;

use crate::standard_json_types::TestInput;
use actix_web::{
    dev::ServiceResponse,
    test,
    test::{read_body, read_body_json, TestRequest},
    App,
};
use blockscout_display_bytes::Bytes as DisplayBytes;
use ethers_solc::artifacts::StandardJsonCompilerInput;
use serde_json::json;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    solidity_verifier_actix::route_solidity_verifier, VerifyResponse,
};
use smart_contract_verifier_server::{Settings, SolidityVerifierService};
use std::{
    collections::BTreeMap,
    fs,
    str::{from_utf8, FromStr},
    sync::Arc,
};
use tokio::sync::{OnceCell, Semaphore};

const CONTRACTS_DIR: &str = "tests/contracts";
const ROUTE: &str = "/api/v1/solidity/verify/standard-json";

async fn global_service() -> &'static Arc<SolidityVerifierService> {
    static SERVICE: OnceCell<Arc<SolidityVerifierService>> = OnceCell::const_new();
    SERVICE
        .get_or_init(|| async {
            let settings = Settings::default();
            let compilers_lock = Semaphore::new(settings.compilers.max_threads.get());
            let service = SolidityVerifierService::new(
                settings.solidity,
                Arc::new(compilers_lock),
                settings.extensions.solidity,
            )
            .await
            .expect("couldn't initialize the service");
            Arc::new(service)
        })
        .await
}

async fn test_setup(dir: &str, input: &mut TestInput) -> (ServiceResponse, Option<DisplayBytes>) {
    let service = global_service().await;
    let app = test::init_service(
        App::new().configure(|config| route_solidity_verifier(config, service.clone())),
    )
    .await;

    let prefix = format!("{}/{}", CONTRACTS_DIR, dir);
    let contract_path = format!("{}/standard_input.json", prefix);
    input.standard_input = Some(input.standard_input.clone().unwrap_or_else(|| {
        fs::read_to_string(&contract_path).expect("Error while reading source")
    }));
    input.creation_tx_input = if !input.ignore_creation_tx_input {
        Some(input.creation_tx_input.clone().unwrap_or_else(|| {
            fs::read_to_string(format!("{}/creation_tx_input", prefix))
                .expect("Error while reading creation_tx_input")
        }))
    } else {
        None
    };
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
        "deployedBytecode": input.deployed_bytecode.as_ref().unwrap(),
        "creationBytecode": input.creation_tx_input.as_ref(),
        "compilerVersion": input.compiler_version,
        "input": input.standard_input
    });

    let response = TestRequest::post()
        .uri(ROUTE)
        .set_json(&request)
        .send_request(&app)
        .await;

    (response, expected_constructor_argument)
}

async fn test_success(dir: &'static str, mut input: TestInput) -> VerifyResponse {
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

    let verification_response: VerifyResponse = read_body_json(response).await;
    let verification_response_clone = verification_response.clone();

    assert_eq!(
        verification_response.status,
        "0", // success
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

    let verification_result_constructor_arguments = verification_result
        .constructor_arguments
        .map(|args| DisplayBytes::from_str(&args).unwrap());
    let expected_constructor_argument = expected_constructor_argument.map(DisplayBytes::from);
    assert_eq!(
        verification_result_constructor_arguments, expected_constructor_argument,
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
        verification_result
            .optimization_runs
            .map(|runs| runs as usize),
        standard_input.settings.optimizer.runs,
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
        .map(|(path, source)| (path.to_str().unwrap().to_string(), source.content))
        .collect();
    assert_eq!(verification_result.sources, sources, "Invalid source");

    verification_response_clone
}

mod success_tests {
    use super::*;

    // Compilers from 0.4.11 to 0.4.18 have output everything regardless of
    // what was specified in outputSelection. That test checks that even in that
    // case resultant output could be parsed successfully.
    #[tokio::test]
    async fn solidity_0_4_11_to_0_4_18() {
        let contract_dir = "solidity_0.4.18";
        let test_input = TestInput::new("Main", "v0.4.18+commit.9cf6e910");
        test_success(contract_dir, test_input).await;
    }

    #[tokio::test]
    async fn yul() {
        let contract_dir = "yul";
        let test_input = TestInput::new("Proxy", "v0.8.7+commit.e28d00a7").set_is_yul();
        test_success(contract_dir, test_input).await;
    }
}

mod regression_tests {
    use super::*;

    // https://github.com/blockscout/blockscout/issues/5748
    #[tokio::test]
    async fn issue_5748() {
        let contract_dir = "issue_5748";
        let test_input = TestInput::new("ExternalTestJson", "v0.6.8+commit.0bbfe453");
        test_success(contract_dir, test_input).await;
    }

    #[tokio::test]
    async fn issue_with_creation_code() {
        let contract_dir = "issue_with_creation_code";
        let test_input =
            TestInput::new("PancakeFactory", "v0.5.16+commit.9c3226ce").has_constructor_args();
        test_success(contract_dir, test_input).await;
    }

    #[tokio::test]
    async fn issue_6275() {
        let contract_dir = "issue_6275";
        let test_input =
            TestInput::new("MultichainProxy", "v0.8.16+commit.07a7930e").ignore_creation_tx_input();
        test_success(contract_dir, test_input).await;
    }
}

mod match_types_tests {
    use super::*;
    use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::verify_response::result::MatchType;
    use crate::standard_json_types::TestInput;
    use crate::test_success;

    fn check_match_type(response: VerifyResponse, expected: MatchType) {
        assert_eq!(
            Into::<i32>::into(expected),
            response
                .result
                .expect("Test succeeded, thus result should exist")
                .match_type,
            "Invalid match type"
        )
    }

    #[tokio::test]
    async fn partial_match() {
        let contract_dir = "match_type_partial";
        let test_input = TestInput::new("Storage", "v0.8.7+commit.e28d00a7");
        let response = test_success(contract_dir, test_input).await;
        check_match_type(response, MatchType::Partial);
    }

    #[tokio::test]
    async fn full_match() {
        let contract_dir = "match_type_full";
        let test_input = TestInput::new("Storage", "v0.8.7+commit.e28d00a7");
        let response = test_success(contract_dir, test_input).await;
        check_match_type(response, MatchType::Full);
    }
}
