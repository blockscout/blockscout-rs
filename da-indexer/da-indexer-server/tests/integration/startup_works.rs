use crate::helpers;
use blockscout_service_launcher::{database, test_server};
use migration::Migrator;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn test_startup_works() {
    let db = database!(Migrator);
    let base = helpers::init_server(db.db_url()).await;
    let response: serde_json::Value = test_server::send_get_request(&base, "/health").await;
    assert_eq!(response, serde_json::json!({"status": "SERVING"}));
}
