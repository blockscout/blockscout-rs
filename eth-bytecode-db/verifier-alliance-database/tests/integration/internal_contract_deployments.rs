use crate::database;
use blockscout_display_bytes::decode_hex;
use pretty_assertions::assert_eq;
use verifier_alliance_database::{internal, ContractDeployment, RetrieveContractDeployment};

const MOD_NAME: &str = "contract_deployments";

#[tokio::test]
async fn insert_regular_deployment_works_and_can_be_retrieved() {
    const TEST_NAME: &str = "insert_regular_deployment_works_and_can_be_retrieved";

    let db_guard = database!();

    let chain_id = 10;
    let address = decode_hex("0x8FbB39A5a79aeCE03c8f13ccEE0b96C128ec1a67").unwrap();
    let transaction_hash =
        decode_hex("0xf4042e19c445551d1059ad3856f83383c48699367cfb3e0edeccd26002dd2292").unwrap();

    let contract_deployment = ContractDeployment::Regular {
        chain_id,
        address: address.clone(),
        transaction_hash: transaction_hash.clone(),
        block_number: 127387809,
        transaction_index: 16,
        deployer: decode_hex("0x1F98431c8aD98523631AE4a59f267346ea31F984").unwrap(),
        creation_code: vec![0x1, 0x2],
        runtime_code: vec![0x3, 0x4],
    };

    let inserted_model =
        internal::insert_contract_deployment(db_guard.client().as_ref(), contract_deployment)
            .await
            .expect("error while inserting");

    /********** retrieval **********/

    let retrieve_contract_deployment =
        RetrieveContractDeployment::regular(chain_id, address, transaction_hash);

    let retrieved_model = internal::retrieve_contract_deployment(
        db_guard.client().as_ref(),
        retrieve_contract_deployment,
    )
    .await
    .expect("error while retrieving")
    .expect("no model has been retrieved");

    assert_eq!(
        inserted_model, retrieved_model,
        "inserted and retrieved models do not match"
    );
}

#[tokio::test]
async fn insert_genesis_deployment_works_and_can_be_retrieved() {
    const TEST_NAME: &str = "insert_genesis_deployment_works_and_can_be_retrieved";

    let db_guard = database!();

    let chain_id = 10;
    let address = decode_hex("0x4200000000000000000000000000000000000008").unwrap();
    let runtime_code = vec![0x3, 0x4];

    let contract_deployment = ContractDeployment::Genesis {
        chain_id: 10,
        address: address.clone(),
        runtime_code: runtime_code.clone(),
    };

    let inserted_model =
        internal::insert_contract_deployment(db_guard.client().as_ref(), contract_deployment)
            .await
            .expect("error while inserting");

    /********** retrieval **********/

    let retrieve_contract_deployment =
        RetrieveContractDeployment::genesis(chain_id, address, runtime_code);

    let retrieved_model = internal::retrieve_contract_deployment(
        db_guard.client().as_ref(),
        retrieve_contract_deployment,
    )
    .await
    .expect("error while retrieving")
    .expect("no model has been retrieved");

    assert_eq!(
        inserted_model, retrieved_model,
        "inserted and retrieved models do not match"
    );
}

#[tokio::test]
async fn non_existed_deployment_retrieval_returns_none() {
    const TEST_NAME: &str = "non_existed_deployment_retrieval_returns_none";

    let db_guard = database!();

    let retrieve_contract_deployment =
        RetrieveContractDeployment::regular(10, vec![0x1], vec![0x1]);

    let retrieved_model = internal::retrieve_contract_deployment(
        db_guard.client().as_ref(),
        retrieve_contract_deployment,
    )
    .await
    .expect("error while retrieving");

    assert_eq!(
        None, retrieved_model,
        "no model was expected to be retrieved"
    );
}
