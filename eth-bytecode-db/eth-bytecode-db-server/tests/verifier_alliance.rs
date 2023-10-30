mod verification_test_helpers;

use async_trait::async_trait;
use blockscout_service_launcher::test_database::TestDbGuard;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use eth_bytecode_db_server::Settings;
use rstest::rstest;
use sea_orm::{
    prelude::{Decimal, Uuid},
    ActiveModelTrait,
    ActiveValue::Set,
    DatabaseConnection, DatabaseTransaction, EntityTrait, TransactionTrait,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;
use std::{collections::HashMap, future::Future, path::PathBuf, str::FromStr, sync::Arc};
use tonic::Response;
use verification_test_helpers::{
    init_db, init_db_raw, init_eth_bytecode_db_server_with_settings_setup, init_verifier_server,
    smart_contract_verifer_mock::{MockSolidityVerifierService, SmartContractVerifierServer},
    verifier_alliance_types::TestCase,
    VerifierService,
};
use verifier_alliance_entity::{
    code, compiled_contracts, contract_deployments, contracts, verified_contracts,
};

#[async_trait]
impl VerifierService<smart_contract_verifier_v2::VerifyResponse> for MockSolidityVerifierService {
    fn add_into_service(&mut self, response: smart_contract_verifier_v2::VerifyResponse) {
        self.expect_verify_standard_json()
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().solidity_service(self)
    }
}

const TEST_SUITE_NAME: &str = "verifier_alliance";

const ROUTE: &str = "/api/v2/verifier/solidity/sources:verify-standard-json";

const API_KEY_NAME: &str = "x-api-key";

const API_KEY: &str = "some api key";

fn verify_request(test_case: &TestCase) -> eth_bytecode_db_v2::VerifySolidityStandardJsonRequest {
    let metadata = eth_bytecode_db_v2::VerificationMetadata {
        chain_id: Some(format!("{}", test_case.chain_id)),
        contract_address: Some(test_case.address.to_string()),
        transaction_hash: Some(test_case.transaction_hash.to_string()),
        block_number: Some(test_case.block_number as i64),
        transaction_index: Some(test_case.transaction_index as i64),
        deployer: Some(test_case.deployer.to_string()),
        creation_code: Some(test_case.deployed_creation_code.to_string()),
        runtime_code: Some(test_case.deployed_runtime_code.to_string()),
    };

    eth_bytecode_db_v2::VerifySolidityStandardJsonRequest {
        bytecode: "".to_string(),
        bytecode_type: eth_bytecode_db_v2::BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        input: "".to_string(),
        metadata: Some(metadata),
    }
}

async fn init_alliance_db(test_suite_name: &str, test_name: &str) -> TestDbGuard {
    let test_name = format!("{test_name}_alliance");
    init_db_raw::<verifier_alliance_migration::Migrator>(test_suite_name, &test_name).await
}

pub struct RequestWrapper<'a, Request> {
    inner: &'a Request,
    headers: reqwest::header::HeaderMap,
}

impl<'a, Request> From<&'a Request> for RequestWrapper<'a, Request> {
    fn from(value: &'a Request) -> Self {
        Self {
            inner: value,
            headers: Default::default(),
        }
    }
}

impl<'a, Request> RequestWrapper<'a, Request> {
    pub fn header(&mut self, key: &str, value: &str) {
        let key = reqwest::header::HeaderName::from_str(key)
            .expect("Error converting key string into header name");
        let value = reqwest::header::HeaderValue::from_str(value)
            .expect("Error converting value string into header value");
        self.headers.insert(key, value);
    }

    pub fn headers(&mut self, headers: HashMap<String, String>) {
        for (key, value) in headers {
            self.header(&key, &value);
        }
    }
}

pub async fn send_request<Request: serde::Serialize, Response: for<'a> serde::Deserialize<'a>>(
    eth_bytecode_db_base: &reqwest::Url,
    route: &str,
    request: &RequestWrapper<'_, Request>,
) -> Response {
    let response = reqwest::Client::new()
        .post(eth_bytecode_db_base.join(route).unwrap())
        .json(&request.inner)
        .headers(request.headers.clone())
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to send request"));

    // Assert that status code is success
    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await.expect("Read body as text");
        panic!("Invalid status code (success expected). Status: {status}. Message: {message}")
    }

    response
        .json()
        .await
        .unwrap_or_else(|_| panic!("Response deserialization failed"))
}

async fn setup<F, Fut: Future<Output = ()>>(
    test_prefix: &str,
    test_case_path: PathBuf,
    setup_db: F,
    is_authorized: bool,
) -> (TestDbGuard, TestCase)
where
    F: FnOnce(Arc<DatabaseConnection>, TestCase) -> Fut,
{
    let service = MockSolidityVerifierService::new();

    // e.g. "tests/alliance_test_cases/full_match.json" => "full_match"
    let test_name = format!(
        "{test_prefix}_{}",
        test_case_path
            .file_stem()
            .as_ref()
            .unwrap()
            .to_str()
            .unwrap()
    );

    let db = init_db(TEST_SUITE_NAME, &test_name).await;
    let alliance_db = init_alliance_db(TEST_SUITE_NAME, &test_name).await;

    let test_case = TestCase::from_file(test_case_path);
    let test_input_data = test_case.to_test_input_data();

    setup_db(alliance_db.client(), test_case.clone()).await;

    let db_url = db.db_url();
    let verifier_addr = init_verifier_server(service, test_input_data.verifier_response).await;

    let settings_setup = |mut settings: Settings| {
        settings.verifier_alliance_database.enabled = true;
        settings.verifier_alliance_database.url = alliance_db.db_url().to_string();

        settings.authorized_keys =
            serde_json::from_value(serde_json::json!({"blockscout": {"key": API_KEY}})).unwrap();
        settings
    };
    let eth_bytecode_db_base =
        init_eth_bytecode_db_server_with_settings_setup(db_url, verifier_addr, settings_setup)
            .await;
    // Fill the database with existing value
    {
        let dummy_request = verify_request(&test_case);
        let mut wrapped_request: RequestWrapper<_> = (&dummy_request).into();
        if is_authorized {
            wrapped_request.header(API_KEY_NAME, API_KEY);
        }
        let _verification_response: eth_bytecode_db_v2::VerifyResponse =
            send_request(&eth_bytecode_db_base, ROUTE, &wrapped_request).await;
    }

    (alliance_db, test_case)
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
pub async fn success_with_existing_deployment(
    #[files("tests/alliance_test_cases/*.json")] test_case_path: PathBuf,
) {
    const TEST_PREFIX: &str = "success_with_existing_deployment";

    let prepare_alliance_database = |db: Arc<DatabaseConnection>, test_case: TestCase| async move {
        let txn = db.begin().await.expect("starting a transaction failed");
        let _contract_deployment_id = insert_contract_deployment(&txn, &test_case).await;
        txn.commit().await.expect("committing transaction failed");
    };

    let (alliance_db, test_case) = setup(
        TEST_PREFIX,
        test_case_path,
        prepare_alliance_database,
        false,
    )
    .await;

    check_compiled_contract(alliance_db.client().as_ref(), &test_case).await;
    check_verified_contract(alliance_db.client().as_ref(), &test_case).await;
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
pub async fn success_without_existing_deployment(
    #[files("tests/alliance_test_cases/full_match.json")] test_case_path: PathBuf,
) {
    const TEST_PREFIX: &str = "success_without_existing_deployment";

    let prepare_alliance_database = |_db: Arc<DatabaseConnection>, _test_case: TestCase| async {};

    let (alliance_db, test_case) =
        setup(TEST_PREFIX, test_case_path, prepare_alliance_database, true).await;

    check_contract_deployment(alliance_db.client().as_ref(), &test_case).await;
    check_compiled_contract(alliance_db.client().as_ref(), &test_case).await;
    check_verified_contract(alliance_db.client().as_ref(), &test_case).await;
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
pub async fn failure_without_existing_deployment_not_authorized(
    #[files("tests/alliance_test_cases/full_match.json")] test_case_path: PathBuf,
) {
    const TEST_PREFIX: &str = "failure_without_existing_deployment_not_authorized";

    let prepare_alliance_database = |_db: Arc<DatabaseConnection>, _test_case: TestCase| async {};

    let (alliance_db, _test_case) = setup(
        TEST_PREFIX,
        test_case_path,
        prepare_alliance_database,
        false,
    )
    .await;

    assert_eq!(
        None,
        retrieve_contract_deployment(alliance_db.client().as_ref()).await,
        "`contract_deployment` inserted"
    );
    assert_eq!(
        None,
        retrieve_compiled_contract(alliance_db.client().as_ref()).await,
        "`compiled_contract` inserted"
    );
    assert_eq!(
        None,
        retrieve_verified_contract(alliance_db.client().as_ref()).await,
        "`verified_contract` inserted"
    );
}

async fn insert_contract_deployment(txn: &DatabaseTransaction, test_case: &TestCase) -> Uuid {
    let contract_id = insert_contract(
        txn,
        test_case.deployed_creation_code.to_vec(),
        test_case.deployed_runtime_code.to_vec(),
    )
    .await;

    contract_deployments::ActiveModel {
        id: Default::default(),
        chain_id: Set(test_case.chain_id.into()),
        address: Set(test_case.address.to_vec()),
        transaction_hash: Set(test_case.transaction_hash.to_vec()),
        block_number: Set(Some(test_case.block_number.into())),
        txindex: Set(Some(test_case.transaction_index.into())),
        deployer: Set(Some(test_case.deployer.to_vec())),
        contract_id: Set(contract_id),
    }
    .insert(txn)
    .await
    .unwrap_or_else(|err| {
        panic!(
            "insertion of a contract deployment failed; \
            contract_id: {contract_id}, \
            err: {err}"
        )
    })
    .id
}

async fn insert_contract(
    txn: &DatabaseTransaction,
    creation_code: Vec<u8>,
    runtime_code: Vec<u8>,
) -> Uuid {
    let creation_code_hash = insert_code(txn, creation_code).await;
    let runtime_code_hash = insert_code(txn, runtime_code).await;

    contracts::ActiveModel {
        id: Default::default(),
        creation_code_hash: Set(creation_code_hash.0.to_vec()),
        runtime_code_hash: Set(runtime_code_hash.0.to_vec()),
    }
    .insert(txn)
    .await
    .unwrap_or_else(|err| {
        panic!(
            "insertion of a contract failed; \
            creation_code_hash: {creation_code_hash}, \
            runtime_code_hash: {runtime_code_hash}, \
            err: {err}"
        )
    })
    .id
}

async fn insert_code(txn: &DatabaseTransaction, code: Vec<u8>) -> keccak_hash::H256 {
    let code_hash = keccak_hash::keccak(&code);
    code::ActiveModel {
        code_hash: Set(code_hash.0.to_vec()),
        code: Set(Some(code)),
    }
    .insert(txn)
    .await
    .unwrap_or_else(|err| panic!("insertion of a code failed; code_hash: {code_hash}, err: {err}"));
    code_hash
}

async fn check_contract(db: &DatabaseConnection, contract: contracts::Model, test_case: &TestCase) {
    let creation_code = code::Entity::find_by_id(contract.creation_code_hash)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        Some(test_case.deployed_creation_code.to_vec()),
        creation_code.code,
        "Invalid creation_code for deployed contract"
    );

    let runtime_code = code::Entity::find_by_id(contract.runtime_code_hash)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        Some(test_case.deployed_runtime_code.to_vec()),
        runtime_code.code,
        "Invalid runtime_code for deployed contract"
    );
}

async fn retrieve_contract_deployment(
    db: &DatabaseConnection,
) -> Option<contract_deployments::Model> {
    contract_deployments::Entity::find()
        .one(db)
        .await
        .expect("Error while retrieving contract deployment")
}
async fn check_contract_deployment(db: &DatabaseConnection, test_case: &TestCase) {
    let contract_deployment = retrieve_contract_deployment(db)
        .await
        .expect("The data has not been added into `contract_deployments` table");

    let test_case_chain_id: Decimal = test_case.chain_id.into();
    let test_case_block_number: Decimal = test_case.block_number.into();
    let test_case_transaction_index: Decimal = test_case.transaction_index.into();
    assert_eq!(
        test_case_chain_id, contract_deployment.chain_id,
        "Invalid contract_deployments.chain_id"
    );
    assert_eq!(
        test_case.address.to_vec(),
        contract_deployment.address,
        "Invalid contract_deployments.address"
    );
    assert_eq!(
        test_case.transaction_hash.to_vec(),
        contract_deployment.transaction_hash,
        "Invalid contract_deployments.transaction_hash"
    );
    assert_eq!(
        Some(test_case_block_number),
        contract_deployment.block_number,
        "Invalid contract_deployments.block_number"
    );
    assert_eq!(
        Some(test_case_transaction_index),
        contract_deployment.txindex,
        "Invalid contract_deployments.txindex"
    );
    assert_eq!(
        Some(test_case.deployer.to_vec()),
        contract_deployment.deployer,
        "Invalid contract_deployments.deployer"
    );

    let contract = contracts::Entity::find_by_id(contract_deployment.contract_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    check_contract(db, contract, test_case).await;
}

async fn retrieve_compiled_contract(db: &DatabaseConnection) -> Option<compiled_contracts::Model> {
    compiled_contracts::Entity::find()
        .one(db)
        .await
        .expect("Error while retrieving compiled contract")
}

async fn check_compiled_contract(db: &DatabaseConnection, test_case: &TestCase) {
    let compiled_contract = retrieve_compiled_contract(db)
        .await
        .expect("The data has not been added into `compiled_contracts` table");

    let test_case_sources = serde_json::to_value(test_case.sources.clone()).unwrap();
    let test_case_creation_code_hash = keccak_hash::keccak(&test_case.compiled_creation_code)
        .0
        .to_vec();
    let test_case_runtime_code_hash = keccak_hash::keccak(&test_case.compiled_runtime_code)
        .0
        .to_vec();

    assert_eq!(
        test_case.compiler, compiled_contract.compiler,
        "Invalid compiler"
    );
    assert_eq!(
        test_case.version, compiled_contract.version,
        "Invalid version"
    );
    assert_eq!(
        test_case.language, compiled_contract.language,
        "Invalid language"
    );
    assert_eq!(test_case.name, compiled_contract.name, "Invalid name");
    assert_eq!(
        test_case.fully_qualified_name, compiled_contract.fully_qualified_name,
        "Invalid fully_qualified_name"
    );
    assert_eq!(
        test_case_sources, compiled_contract.sources,
        "Invalid sources"
    );
    assert_eq!(
        test_case.compiler_settings, compiled_contract.compiler_settings,
        "Invalid compiler_settings"
    );
    assert_eq!(
        test_case.compilation_artifacts, compiled_contract.compilation_artifacts,
        "Invalid compilation_artifacts"
    );
    assert_eq!(
        test_case_creation_code_hash, compiled_contract.creation_code_hash,
        "Invalid creation_code_hash"
    );
    assert_eq!(
        test_case.creation_code_artifacts, compiled_contract.creation_code_artifacts,
        "Invalid creation_code_artifacts"
    );
    assert_eq!(
        test_case_runtime_code_hash, compiled_contract.runtime_code_hash,
        "Invalid runtime_code_hash"
    );
    assert_eq!(
        test_case.runtime_code_artifacts, compiled_contract.runtime_code_artifacts,
        "Invalid runtime_code_artifacts"
    );
}

async fn retrieve_verified_contract(db: &DatabaseConnection) -> Option<verified_contracts::Model> {
    verified_contracts::Entity::find()
        .one(db)
        .await
        .expect("Error while retrieving verified contract")
}

async fn check_verified_contract(db: &DatabaseConnection, test_case: &TestCase) {
    let verified_contract = retrieve_verified_contract(db)
        .await
        .expect("The data has not been added into `verified_contracts` table");

    let test_case_creation_values = Some(test_case.creation_values.clone());
    let test_case_creation_transformations = Some(test_case.creation_transformations.clone());
    let test_case_runtime_values = Some(test_case.runtime_values.clone());
    let test_case_runtime_transformations = Some(test_case.runtime_transformations.clone());

    assert_eq!(
        test_case.creation_match, verified_contract.creation_match,
        "Invalid creation_match"
    );
    assert_eq!(
        test_case_creation_values, verified_contract.creation_values,
        "Invalid creation_values"
    );
    assert_eq!(
        test_case_creation_transformations, verified_contract.creation_transformations,
        "Invalid creation_transformations"
    );
    assert_eq!(
        test_case.runtime_match, verified_contract.runtime_match,
        "Invalid runtime_match"
    );
    assert_eq!(
        test_case_runtime_values, verified_contract.runtime_values,
        "Invalid runtime_values"
    );
    assert_eq!(
        test_case_runtime_transformations, verified_contract.runtime_transformations,
        "Invalid runtime_transformations"
    );
}
