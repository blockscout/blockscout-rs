use crate::database;
use verifier_alliance_database::{ContractCode};

const MOD_NAME: &str = "contracts";

#[tokio::test]
async fn insert_complete_code_works() {
    const TEST_NAME: &str = "insert_complete_code_works";

    let db_guard = database!();

    let contract_code = ContractCode::CompleteCode { creation_code: vec![0x1, 0x2], runtime_code: vec![0x3, 0x4] };

    let _model = verifier_alliance_database::insert_contract(db_guard.client().as_ref(), contract_code).await
        .expect("error while inserting");
}

#[tokio::test]
async fn insert_only_creation_code_works() {
    const TEST_NAME: &str = "insert_only_creation_code_works";

    let db_guard = database!();

    let contract_code = ContractCode::OnlyCreationCode { code: vec![0x1, 0x2] };

    let _model = verifier_alliance_database::insert_contract(db_guard.client().as_ref(), contract_code).await
        .expect("error while inserting");
}


#[tokio::test]
async fn insert_only_runtime_code_works() {
    const TEST_NAME: &str = "insert_only_runtime_code_works";

    let db_guard = database!();

    let contract_code = ContractCode::OnlyRuntimeCode{ code: vec![0x3, 0x4] };

    let _model = verifier_alliance_database::insert_contract(db_guard.client().as_ref(), contract_code).await
        .expect("error while inserting");
}
