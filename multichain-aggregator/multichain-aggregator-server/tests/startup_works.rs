mod helpers;

use blockscout_service_launcher::{database, test_server};
use migration::Migrator;
use pretty_assertions::assert_eq;

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_startup_works() {
    let db = database!(Migrator);
    let db_url = db.db_url();
    let base = helpers::init_multichain_aggregator_server(db_url, |x| x).await;
    let response: serde_json::Value = test_server::send_get_request(&base, "/health").await;
    assert_eq!(response, serde_json::json!({"status": "SERVING"}));
}
