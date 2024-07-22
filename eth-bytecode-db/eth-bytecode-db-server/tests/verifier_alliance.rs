mod verification_test_helpers;

use pretty_assertions::assert_eq;
use rstest::rstest;
use sea_orm::{
    prelude::{Decimal, Uuid},
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter,
    TransactionTrait,
};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc};
use verification_test_helpers::{
    verifier_alliance_setup::{Setup, SetupData},
    verifier_alliance_types::TestCase,
};
use verifier_alliance_entity::{
    code, compiled_contracts, contract_deployments, contracts, verified_contracts,
};

const TEST_SUITE_NAME: &str = "verifier_alliance";

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

    let SetupData {
        alliance_db,
        test_case,
        ..
    } = Setup::new(TEST_PREFIX)
        .setup_db(prepare_alliance_database)
        .setup(TEST_SUITE_NAME, test_case_path)
        .await;

    let contract_deployment =
        retrieve_contract_deployment(alliance_db.client().as_ref(), Some(&test_case))
            .await
            .expect("contract deployment does not exist");

    let compiled_contract =
        check_compiled_contract(alliance_db.client().as_ref(), &test_case).await;
    check_verified_contract(
        alliance_db.client().as_ref(),
        &test_case,
        &contract_deployment,
        &compiled_contract,
    )
    .await;
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
pub async fn success_without_existing_deployment(
    #[files("tests/alliance_test_cases/*.json")] test_case_path: PathBuf,
) {
    const TEST_PREFIX: &str = "success_without_existing_deployment";

    let SetupData {
        alliance_db,
        test_case,
        ..
    } = Setup::new(TEST_PREFIX)
        .authorized()
        .setup(TEST_SUITE_NAME, test_case_path)
        .await;

    let contract_deployment =
        check_contract_deployment(alliance_db.client().as_ref(), &test_case).await;
    let compiled_contract =
        check_compiled_contract(alliance_db.client().as_ref(), &test_case).await;
    check_verified_contract(
        alliance_db.client().as_ref(),
        &test_case,
        &contract_deployment,
        &compiled_contract,
    )
    .await;
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
pub async fn failure_without_existing_deployment_not_authorized(
    #[files("tests/alliance_test_cases/*.json")] test_case_path: PathBuf,
) {
    const TEST_PREFIX: &str = "failure_without_existing_deployment_not_authorized";

    let SetupData { alliance_db, .. } = Setup::new(TEST_PREFIX)
        .setup(TEST_SUITE_NAME, test_case_path)
        .await;

    assert_eq!(
        None,
        retrieve_contract_deployment(alliance_db.client().as_ref(), None).await,
        "`contract_deployment` inserted"
    );
    assert_eq!(
        None,
        retrieve_compiled_contract(alliance_db.client().as_ref(), None).await,
        "`compiled_contract` inserted"
    );
    assert_eq!(
        None,
        retrieve_verified_contract(alliance_db.client().as_ref(), None, None).await,
        "`verified_contract` inserted"
    );
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
pub async fn verification_with_different_sources(
    #[files("tests/alliance_test_cases/full_match.json")] full_match_path: PathBuf,
    #[files("tests/alliance_test_cases/partial_match.json")] partial_match_1_path: PathBuf,
    #[files("tests/alliance_test_cases/partial_match_2.json")] partial_match_2_path: PathBuf,
) {
    const TEST_PREFIX: &str = "verification_with_different_sources";

    let mut setup = Setup::new(TEST_PREFIX).authorized();

    /********** Add first partial matched contract **********/
    let SetupData {
        alliance_db,
        test_case: partial_match_1_test_case,
        ..
    } = setup.setup(TEST_SUITE_NAME, partial_match_1_path).await;
    let db_client_owned = alliance_db.client();
    let db_client = db_client_owned.as_ref();

    let contract_deployments_partial_match_1 = retrieve_contract_deployments(db_client).await;
    assert_eq!(
        1,
        contract_deployments_partial_match_1.len(),
        "Invalid number of contract deployments"
    );

    let compiled_contracts_partial_match_1 = retrieve_compiled_contracts(db_client).await;
    assert_eq!(
        1,
        compiled_contracts_partial_match_1.len(),
        "Invalid number of compiled contracts"
    );

    let verified_contracts_partial_match_1 = retrieve_verified_contracts(db_client).await;
    assert_eq!(
        1,
        verified_contracts_partial_match_1.len(),
        "Invalid number of verified contracts"
    );

    /********** Add second partial matched contract (should not be added) **********/
    setup = setup.alliance_db(alliance_db.clone());
    setup.setup(TEST_SUITE_NAME, partial_match_2_path).await;

    let contract_deployments_partial_match_2 = retrieve_contract_deployments(db_client).await;
    assert_eq!(
        contract_deployments_partial_match_1, contract_deployments_partial_match_2,
        "Invalid contract deployments after second partial match insertion"
    );

    let compiled_contracts_partial_match_2 = retrieve_compiled_contracts(db_client).await;
    assert_eq!(
        compiled_contracts_partial_match_1, compiled_contracts_partial_match_2,
        "Invalid compiled contracts after second partial match insertion"
    );

    let verified_contracts_partial_match_2 = retrieve_verified_contracts(db_client).await;
    assert_eq!(
        verified_contracts_partial_match_1, verified_contracts_partial_match_2,
        "Invalid verified contracts after second partial match insertion"
    );

    /********** Add full matched contract **********/
    let SetupData {
        test_case: full_match_test_case,
        ..
    } = setup.setup(TEST_SUITE_NAME, full_match_path).await;

    let contract_deployment_partial_match_1 =
        check_contract_deployment(db_client, &partial_match_1_test_case).await;
    let contract_deployment_full_match =
        check_contract_deployment(db_client, &full_match_test_case).await;

    let compiled_contract_partial_match_1 =
        check_compiled_contract(db_client, &partial_match_1_test_case).await;
    let compiled_contract_full_match =
        check_compiled_contract(db_client, &full_match_test_case).await;

    check_verified_contract(
        db_client,
        &partial_match_1_test_case,
        &contract_deployment_partial_match_1,
        &compiled_contract_partial_match_1,
    )
    .await;
    check_verified_contract(
        db_client,
        &full_match_test_case,
        &contract_deployment_full_match,
        &compiled_contract_full_match,
    )
    .await;
}

async fn insert_contract_deployment(txn: &DatabaseTransaction, test_case: &TestCase) -> Uuid {
    let contract_id = insert_contract(
        txn,
        test_case.deployed_creation_code.as_deref().map(Vec::from),
        test_case.deployed_runtime_code.to_vec(),
    )
    .await;

    contract_deployments::ActiveModel {
        id: Default::default(),
        created_at: Default::default(),
        updated_at: Default::default(),
        created_by: Default::default(),
        updated_by: Default::default(),
        chain_id: Set(test_case.chain_id.into()),
        address: Set(test_case.address.to_vec()),
        transaction_hash: Set(test_case.transaction_hash.to_vec()),
        block_number: Set(test_case.block_number.into()),
        transaction_index: Set(test_case.transaction_index.into()),
        deployer: Set(test_case.deployer.to_vec()),
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
    creation_code: Option<Vec<u8>>,
    runtime_code: Vec<u8>,
) -> Uuid {
    let creation_code_hash = match creation_code {
        Some(creation_code) => insert_code(txn, creation_code).await,
        None => Vec::new(),
    };
    let runtime_code_hash = insert_code(txn, runtime_code).await;

    contracts::ActiveModel {
        id: Default::default(),
        created_at: Default::default(),
        updated_at: Default::default(),
        created_by: Default::default(),
        updated_by: Default::default(),
        creation_code_hash: Set(creation_code_hash.clone()),
        runtime_code_hash: Set(runtime_code_hash.clone()),
    }
    .insert(txn)
    .await
    .unwrap_or_else(|err| {
        panic!(
            "insertion of a contract failed; \
            creation_code_hash: {}, \
            runtime_code_hash: {}, \
            err: {err}",
            blockscout_display_bytes::Bytes::from(creation_code_hash),
            blockscout_display_bytes::Bytes::from(runtime_code_hash),
        )
    })
    .id
}

async fn insert_code(txn: &DatabaseTransaction, code: Vec<u8>) -> Vec<u8> {
    let code_hash = Sha256::digest(&code).to_vec();
    let code_hash_keccak = keccak_hash::keccak(&code).0.to_vec();
    code::ActiveModel {
        code_hash: Set(code_hash.clone()),
        created_at: Default::default(),
        updated_at: Default::default(),
        created_by: Default::default(),
        updated_by: Default::default(),
        code_hash_keccak: Set(code_hash_keccak),
        code: Set(Some(code)),
    }
    .insert(txn)
    .await
    .unwrap_or_else(|err| {
        panic!(
            "insertion of a code failed; code_hash: {}, err: {err}",
            hex::encode(&code_hash)
        )
    });
    code_hash
}

async fn check_contract(db: &DatabaseConnection, contract: contracts::Model, test_case: &TestCase) {
    let creation_code = code::Entity::find_by_id(contract.creation_code_hash)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        test_case.deployed_creation_code.as_deref().map(Vec::from),
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

async fn retrieve_contract_deployments(
    db: &DatabaseConnection,
) -> Vec<contract_deployments::Model> {
    contract_deployments::Entity::find()
        .all(db)
        .await
        .expect("Error while retrieving contract deployments")
}

// `test_case` is None if we would like to retrieve any model
// (useful when want to check that no data has been inserted)
async fn retrieve_contract_deployment(
    db: &DatabaseConnection,
    test_case: Option<&TestCase>,
) -> Option<contract_deployments::Model> {
    let mut query = contract_deployments::Entity::find();
    if let Some(test_case) = test_case {
        query = query
            .filter(contract_deployments::Column::ChainId.eq(Decimal::from(test_case.chain_id)))
            .filter(contract_deployments::Column::Address.eq(test_case.address.to_vec()))
            .filter(
                contract_deployments::Column::TransactionHash
                    .eq(test_case.transaction_hash.to_vec()),
            );
    }
    query
        .one(db)
        .await
        .expect("Error while retrieving contract deployment")
}
async fn check_contract_deployment(
    db: &DatabaseConnection,
    test_case: &TestCase,
) -> contract_deployments::Model {
    let contract_deployment = retrieve_contract_deployment(db, Some(test_case))
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
        test_case_block_number, contract_deployment.block_number,
        "Invalid contract_deployments.block_number"
    );
    assert_eq!(
        test_case_transaction_index, contract_deployment.transaction_index,
        "Invalid contract_deployments.transaction_index"
    );
    assert_eq!(
        test_case.deployer.to_vec(),
        contract_deployment.deployer,
        "Invalid contract_deployments.deployer"
    );

    let contract = contracts::Entity::find_by_id(contract_deployment.contract_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    check_contract(db, contract, test_case).await;

    contract_deployment
}

async fn retrieve_compiled_contracts(db: &DatabaseConnection) -> Vec<compiled_contracts::Model> {
    compiled_contracts::Entity::find()
        .all(db)
        .await
        .expect("Error while retrieving compiled contracts")
}

async fn retrieve_compiled_contract(
    db: &DatabaseConnection,
    test_case: Option<&TestCase>,
) -> Option<compiled_contracts::Model> {
    let mut query = compiled_contracts::Entity::find();
    if let Some(test_case) = test_case {
        let creation_code_hash = keccak_hash::keccak(&test_case.compiled_creation_code);
        let runtime_code_hash = keccak_hash::keccak(&test_case.compiled_runtime_code);
        query = query
            .filter(compiled_contracts::Column::Compiler.eq(test_case.compiler.clone()))
            .filter(compiled_contracts::Column::Language.eq(test_case.language.clone()))
            .filter(compiled_contracts::Column::CreationCodeHash.eq(creation_code_hash.0.to_vec()))
            .filter(compiled_contracts::Column::RuntimeCodeHash.eq(runtime_code_hash.0.to_vec()))
    }
    query
        .one(db)
        .await
        .expect("Error while retrieving compiled contract")
}

async fn check_compiled_contract(
    db: &DatabaseConnection,
    test_case: &TestCase,
) -> compiled_contracts::Model {
    let compiled_contract = retrieve_compiled_contract(db, Some(test_case))
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

    compiled_contract
}

async fn retrieve_verified_contracts(db: &DatabaseConnection) -> Vec<verified_contracts::Model> {
    verified_contracts::Entity::find()
        .all(db)
        .await
        .expect("Error while retrieving verified contracts")
}

async fn retrieve_verified_contract(
    db: &DatabaseConnection,
    contract_deployment: Option<&contract_deployments::Model>,
    compiled_contract: Option<&compiled_contracts::Model>,
) -> Option<verified_contracts::Model> {
    let mut query = verified_contracts::Entity::find();
    if let Some(contract_deployment) = contract_deployment {
        query = query.filter(verified_contracts::Column::DeploymentId.eq(contract_deployment.id))
    }
    if let Some(compiled_contract) = compiled_contract {
        query = query.filter(verified_contracts::Column::CompilationId.eq(compiled_contract.id))
    }
    query
        .one(db)
        .await
        .expect("Error while retrieving verified contract")
}

async fn check_verified_contract(
    db: &DatabaseConnection,
    test_case: &TestCase,
    contract_deployment: &contract_deployments::Model,
    compiled_contract: &compiled_contracts::Model,
) -> verified_contracts::Model {
    let verified_contract =
        retrieve_verified_contract(db, Some(contract_deployment), Some(compiled_contract))
            .await
            .expect("The data has not been added into `verified_contracts` table");

    assert_eq!(
        test_case.creation_match, verified_contract.creation_match,
        "Invalid creation_match"
    );
    assert_eq!(
        test_case.creation_values, verified_contract.creation_values,
        "Invalid creation_values"
    );
    assert_eq!(
        test_case.creation_transformations, verified_contract.creation_transformations,
        "Invalid creation_transformations"
    );
    assert_eq!(
        test_case.runtime_match, verified_contract.runtime_match,
        "Invalid runtime_match"
    );
    assert_eq!(
        test_case.runtime_values, verified_contract.runtime_values,
        "Invalid runtime_values"
    );
    assert_eq!(
        test_case.runtime_transformations, verified_contract.runtime_transformations,
        "Invalid runtime_transformations"
    );

    verified_contract
}
