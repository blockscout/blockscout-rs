use crate::database;

const MOD_NAME: &str = "code";

#[tokio::test]
async fn insert_code_works() {
    const TEST_NAME: &str = "insert_code_works";

    let db_guard = database!();

    let code = vec![0x01, 0x02, 0x03, 0x04];
    let _model = verifier_alliance_database::insert_code(db_guard.client().as_ref(), code)
        .await
        .expect("error while inserting");
}
