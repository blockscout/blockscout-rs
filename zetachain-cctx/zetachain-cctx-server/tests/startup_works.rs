mod helpers;

use std::sync::Arc;

use blockscout_service_launcher::test_server;
use pretty_assertions::assert_eq;

use zetachain_cctx_logic::client::{Client, RpcSettings};


#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_historical_sync() {
    let db = helpers::init_db(
        "test",
        "startup_works",
    )
    .await;
    let db_url = db.db_url();

    
    let client = Client::new(RpcSettings::default());
    let base = helpers::init_zetachain_cctx_server(
        db_url,
        |x| x,
        db.client(),
        Arc::new(client),
    ).await;
    let response: serde_json::Value = test_server::send_get_request(&base, "/health")
                .await;
    assert_eq!(response, serde_json::json!({"status": "SERVING"}));
}