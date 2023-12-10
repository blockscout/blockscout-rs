mod verification_test_helpers;

use crate::verification_test_helpers::test_input_data::TestInputData;
use async_trait::async_trait;
use blockscout_service_launcher::test_database::TestDbGuard;
use eth_bytecode_db::verification::{
    solidity_standard_json, solidity_standard_json::StandardJson, Client, Error, Source,
    SourceType, VerificationMetadata, VerificationRequest,
};
use pretty_assertions::assert_eq;
use rstest::rstest;
use sea_orm::{
    prelude::{Decimal, Uuid},
    ActiveModelTrait,
    ActiveValue::Set,
    DatabaseConnection, DatabaseTransaction, EntityTrait, TransactionTrait,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    VerifyResponse, VerifySolidityStandardJsonRequest,
};
use std::{future::Future, path::PathBuf, sync::Arc};
use verification_test_helpers::{
    init_db,
    smart_contract_veriifer_mock::{MockSolidityVerifierService, SmartContractVerifierServer},
    start_server_and_init_client,
    verifier_alliance_types::TestCase,
    VerifierService,
};
use verifier_alliance_entity::{
    code, compiled_contracts, contract_deployments, contracts, verified_contracts,
};

const DB_PREFIX: &str = "verifier_alliance";

#[async_trait]
impl VerifierService<VerificationRequest<StandardJson>> for MockSolidityVerifierService {
    type GrpcT = VerifySolidityStandardJsonRequest;

    fn add_into_service(
        &mut self,
        request: VerifySolidityStandardJsonRequest,
        response: VerifyResponse,
    ) {
        self.expect_verify_standard_json()
            .withf(move |arg| arg.get_ref() == &request)
            .returning(move |_| Ok(tonic::Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().solidity_service(self)
    }

    fn generate_request(
        &self,
        _id: u8,
        _metadata: Option<VerificationMetadata>,
    ) -> VerificationRequest<StandardJson> {
        unreachable!()
        // generate_verification_request(id, default_request_content(), metadata)
    }

    fn source_type(&self) -> SourceType {
        unreachable!()
    }

    async fn verify(
        client: Client,
        request: VerificationRequest<StandardJson>,
    ) -> Result<Source, Error> {
        solidity_standard_json::verify(client, request).await
    }
}

pub async fn init_alliance_db(db_prefix: &str, test_name: &str) -> TestDbGuard {
    let db_name = format!("{db_prefix}_{test_name}_alliance");
    TestDbGuard::new::<verifier_alliance_migration::Migrator>(db_name.as_str()).await
}

async fn setup<F, Fut: Future<Output = ()>>(
    test_prefix: &str,
    test_case_path: PathBuf,
    setup_db: F,
    is_authorized: bool,
) -> (Client, TestCase)
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

    let test_case = TestCase::from_file(test_case_path);
    let input_data = input_data(&test_case, is_authorized);

    let db = init_db(DB_PREFIX, &test_name).await;
    let alliance_db = init_alliance_db(DB_PREFIX, &test_name).await;

    setup_db(alliance_db.client(), test_case.clone()).await;

    let client = start_server_and_init_client(db.client(), service, vec![input_data.clone()])
        .await
        .with_alliance_db_arc(alliance_db.client());

    let _source =
        MockSolidityVerifierService::verify(client.clone(), input_data.eth_bytecode_db_request)
            .await
            .expect("verification failed");

    (client, test_case)
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

    let (client, test_case) = setup(
        TEST_PREFIX,
        test_case_path,
        prepare_alliance_database,
        false,
    )
    .await;
    let alliance_db_client = client.alliance_db_client.unwrap();

    check_compiled_contract(alliance_db_client.as_ref(), &test_case).await;
    check_verified_contract(alliance_db_client.as_ref(), &test_case).await;
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
pub async fn success_without_existing_deployment(
    #[files("tests/alliance_test_cases/full_match.json")] test_case_path: PathBuf,
) {
    const TEST_PREFIX: &str = "success_without_existing_deployment";

    let prepare_alliance_database = |_db: Arc<DatabaseConnection>, _test_case: TestCase| async {};

    let (client, test_case) =
        setup(TEST_PREFIX, test_case_path, prepare_alliance_database, true).await;
    let alliance_db_client = client.alliance_db_client.unwrap();

    check_contract_deployment(alliance_db_client.as_ref(), &test_case).await;
    check_compiled_contract(alliance_db_client.as_ref(), &test_case).await;
    check_verified_contract(alliance_db_client.as_ref(), &test_case).await;
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
pub async fn failure_without_existing_deployment_not_authorized(
    #[files("tests/alliance_test_cases/full_match.json")] test_case_path: PathBuf,
) {
    const TEST_PREFIX: &str = "failure_without_existing_deployment_not_authorized";

    let prepare_alliance_database = |_db: Arc<DatabaseConnection>, _test_case: TestCase| async {};

    let (client, _test_case) = setup(
        TEST_PREFIX,
        test_case_path,
        prepare_alliance_database,
        false,
    )
    .await;
    let alliance_db_client = client.alliance_db_client.unwrap();

    assert_eq!(
        None,
        retrieve_contract_deployment(alliance_db_client.as_ref()).await,
        "`contract_deployment` inserted"
    );
    assert_eq!(
        None,
        retrieve_compiled_contract(alliance_db_client.as_ref()).await,
        "`compiled_contract` inserted"
    );
    assert_eq!(
        None,
        retrieve_verified_contract(alliance_db_client.as_ref()).await,
        "`verified_contract` inserted"
    );
}

fn input_data(
    test_case: &TestCase,
    is_authorized: bool,
) -> TestInputData<VerificationRequest<StandardJson>> {
    test_case.to_test_input_data(
        StandardJson {
            input: "".to_string(),
        },
        is_authorized,
    )
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
