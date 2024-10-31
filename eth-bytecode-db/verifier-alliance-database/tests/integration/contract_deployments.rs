use crate::database;
use blockscout_display_bytes::decode_hex;
use verifier_alliance_database::ContractDeployment;

const MOD_NAME: &str = "contract_deployments";

#[tokio::test]
async fn insert_regular_deployment_works() {
    const TEST_NAME: &str = "insert_regular_deployment_works";

    let db_guard = database!();

    let contract_deployment = ContractDeployment::Regular {
        chain_id: 10,
        address: decode_hex("0x8FbB39A5a79aeCE03c8f13ccEE0b96C128ec1a67").unwrap(),
        transaction_hash: decode_hex(
            "0xf4042e19c445551d1059ad3856f83383c48699367cfb3e0edeccd26002dd2292",
        )
        .unwrap(),
        block_number: 127387809,
        transaction_index: 16,
        deployer: decode_hex("0x1F98431c8aD98523631AE4a59f267346ea31F984").unwrap(),
        creation_code: vec![0x1, 0x2],
        runtime_code: vec![0x3, 0x4],
    };

    let _model = verifier_alliance_database::insert_contract_deployment(
        db_guard.client().as_ref(),
        contract_deployment,
    )
    .await
    .expect("error while inserting");
}

#[tokio::test]
async fn insert_genesis_deployment_works() {
    const TEST_NAME: &str = "insert_genesis_deployment_works";

    let db_guard = database!();

    let contract_deployment = ContractDeployment::Genesis {
        chain_id: 10,
        address: decode_hex("0x4200000000000000000000000000000000000008").unwrap(),
        runtime_code: vec![0x3, 0x4],
    };

    let _model = verifier_alliance_database::insert_contract_deployment(
        db_guard.client().as_ref(),
        contract_deployment,
    )
    .await
    .expect("error while inserting");
}
