mod smart_contract_veriifer_mock;

use eth_bytecode_db::verification::Client;
use sea_orm::DatabaseConnection;

#[tokio::test]
async fn test_data_is_added_into_database() {
    let db_client = DatabaseConnection::default();

    // let server = 

    // let client = Client::new(db_client, )
}
