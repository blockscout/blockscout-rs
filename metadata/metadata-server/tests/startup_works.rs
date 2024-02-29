mod helpers;

use blockscout_service_launcher::test_server;
use pretty_assertions::assert_eq;

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_startup_works() {
    let db = helpers::init_db("test", "startup_works").await;
    let db_url = db.db_url();
    let base = helpers::init_metadata_server(db_url, |x| x).await;
    let response: serde_json::Value = test_server::send_get_request(&base, "/health").await;
    assert_eq!(response, serde_json::json!({"status": "SERVING"}));
}
