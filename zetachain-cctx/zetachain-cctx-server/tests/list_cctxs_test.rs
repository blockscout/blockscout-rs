mod helpers;
use blockscout_service_launcher::test_server;
use uuid::Uuid;
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::{CctxListItem};
use std::sync::Arc;
use zetachain_cctx_logic::{client::{Client, RpcSettings}, database::ZetachainCctxDatabase, models::CrossChainTx};
use sea_orm::TransactionTrait;
// use crate::helpers::{init_db, init_zetachain_cctx_server};

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_list_cctxs_endpoint() {
    let db = crate::helpers::init_db("test", "list_cctxs").await;
    let db_url = db.db_url();

    let client = Client::new(RpcSettings::default());
    let base = crate::helpers::init_zetachain_cctx_server(
        db_url,
        |mut x| {
            x.indexer.enabled = false;
            x.tracing.enabled = false;
            x
        },
        db.client(),
        Arc::new(client),
    )
    .await;

    let dummy_cctxs: Vec<CrossChainTx> = vec!["test_list_cctxs_endpoint_1"]
        .iter()
        .map(|x| 
            crate::helpers::dummy_cross_chain_tx(x, "PendingOutbound")
        )
        
        .collect();

    let database = ZetachainCctxDatabase::new(db.client());

    let tx = db.client().begin().await.unwrap();
    database.batch_insert_transactions(Uuid::new_v4(), &dummy_cctxs, &tx).await.unwrap();
    tx.commit().await.unwrap();

    // Test the ListCctxs endpoint
    let response: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/CctxInfoService:list?limit=10&offset=0")
            .await;

    // The response should be a valid JSON object with a "cctxs" array
    assert!(response.is_object());
    assert!(response.get("cctxs").is_some());
    assert!(response.get("cctxs").unwrap().is_array());
    let cctxs: Vec<CctxListItem> = serde_json::from_value(response.get("cctxs").unwrap().clone()).unwrap();
    assert_eq!(cctxs.len(), 1);
    assert_eq!(cctxs[0].index, "test_list_cctxs_endpoint_1");
    assert_eq!(cctxs[0].status, 1);
    assert_eq!(cctxs[0].amount, "42691234567890,1000000000000000000");
    assert_eq!(cctxs[0].source_chain_id, "1");
    assert_eq!(cctxs[0].target_chain_id, "3,2");
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_list_cctxs_with_status_filter() {
    let db = crate::helpers::init_db("test", "list_cctxs_with_status_filter").await;
    let db_url = db.db_url();

    let client = Client::new(RpcSettings::default());
    let base =
        crate::helpers::init_zetachain_cctx_server(db_url, |mut x| {
            x.tracing.enabled = false;
            x.indexer.enabled = false;
            x
        }, db.client(), Arc::new(client))
            .await;

    let dummy_cctxs: Vec<CrossChainTx> = vec!["test_list_cctxs_with_status_filter_1", "test_list_cctxs_with_status_filter_2"]
    .iter()
    .map(|x| 
        crate::helpers::dummy_cross_chain_tx(x, "PendingInbound")
    )
    .chain(vec!["test_list_cctxs_with_status_filter_3", "test_list_cctxs_with_status_filter_4", "test_list_cctxs_with_status_filter_5"]
    .iter()
    .map(|x| 
        crate::helpers::dummy_cross_chain_tx(x, "PendingOutbound")
    ))
    .collect();

    let database = ZetachainCctxDatabase::new(db.client());
    let tx = db.client().begin().await.unwrap();
    database.batch_insert_transactions(Uuid::new_v4(), &dummy_cctxs, &tx).await.unwrap();
    tx.commit().await.unwrap();

    // Test the ListCctxs endpoint with status filter
    let response: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/CctxInfoService:list?limit=10&offset=0&status=PendingInbound",
    )
    .await;

    // The response should be a valid JSON object with a "cctxs" array
    assert!(response.is_object());

    let cctxs = response.get("cctxs");

    assert!(cctxs.is_some());
    assert!(cctxs.unwrap().is_array());

    let cctxs = cctxs.unwrap().as_array().unwrap();

    assert_eq!(cctxs.len(), 2);


    let response: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/CctxInfoService:list?limit=10&offset=0&status=PendingOutbound",
    )
    .await;

    // The response should be a valid JSON object with a "cctxs" array
    assert!(response.is_object());

    let cctxs = response.get("cctxs");

    assert!(cctxs.is_some());
    assert!(cctxs.unwrap().is_array());

    let cctxs = cctxs.unwrap().as_array().unwrap();

    assert_eq!(cctxs.len(), 3);


    let response: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/CctxInfoService:list?limit=2&offset=0&status=PendingOutbound",
    )
    .await;

    // The response should be a valid JSON object with a "cctxs" array
    assert!(response.is_object());

    let cctxs = response.get("cctxs");

    assert!(cctxs.is_some());
    assert!(cctxs.unwrap().is_array());

    let cctxs = cctxs.unwrap().as_array().unwrap();

    assert_eq!(cctxs.len(), 2);


    
}
