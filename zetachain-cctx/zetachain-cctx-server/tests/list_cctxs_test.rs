mod helpers;
use blockscout_service_launcher::{test_server, tracing::{JaegerSettings, TracingSettings}};
use uuid::Uuid;
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::{CctxListItem, ListCctxsResponse};
use std::sync::Arc;
use zetachain_cctx_logic::{client::{Client, RpcSettings}, database::ZetachainCctxDatabase, models::{ CoinType, CrossChainTx}};
use sea_orm::TransactionTrait;
use blockscout_service_launcher::tracing::init_logs;
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
            x.websocket.enabled = false;
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
        test_server::send_get_request(&base, "/api/v1/CctxInfo:list?limit=10&offset=0")
            .await;

    // The response should be a valid JSON object with a "cctxs" array
    assert!(response.is_object());
    assert!(response.get("items").is_some());
    assert!(response.get("items").unwrap().is_array());
    let cctxs: Vec<CctxListItem> = serde_json::from_value(response.get("items").unwrap().clone()).unwrap();
    assert_eq!(cctxs.len(), 1);
    assert_eq!(cctxs[0].index, "test_list_cctxs_endpoint_1");
    assert_eq!(cctxs[0].status, 1);
    assert_eq!(cctxs[0].amount, "8504");
    assert_eq!(cctxs[0].source_chain_id, "1");
    assert_eq!(cctxs[0].target_chain_id, "2");
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_list_cctxs_with_status_filter() {
    if std::env::var("TEST_TRACING").is_ok() {
        init_logs("test_list_cctxs_with_status_filter",
         &TracingSettings::default(),
          &JaegerSettings::default()).unwrap();
    }

    let db = crate::helpers::init_db("test", "list_cctxs_with_status_filter").await;
    let db_url = db.db_url();

    let client = Client::new(RpcSettings::default());
    let base =
        crate::helpers::init_zetachain_cctx_server(db_url, |mut x| {
            x.tracing.enabled = false;
            x.indexer.enabled = false;
            x.websocket.enabled = false;
            x
        }, db.client(), Arc::new(client))
            .await;

    let dummy_cctxs: Vec<CrossChainTx> = vec!["test_list_cctxs_with_status_filter_1", "test_list_cctxs_with_status_filter_2"]
    .iter()
    .map(|x| 
        crate::helpers::dummy_cross_chain_tx(x, "OutboundMined")
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
        "/api/v1/CctxInfo:list?limit=10&offset=0&status_reduced=Success",
    )
    .await;

    // The response should be a valid JSON object with a "cctxs" array
    assert!(response.is_object());

    let cctxs = response.get("items");

    assert!(cctxs.is_some());
    assert!(cctxs.unwrap().is_array());

    let cctxs = cctxs.unwrap().as_array().unwrap();

    assert_eq!(cctxs.len(), 2);


    let response: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/CctxInfo:list?limit=10&offset=0&status_reduced=Pending",
    )
    .await;

    // The response should be a valid JSON object with a "cctxs" array
    assert!(response.is_object());

    let cctxs = response.get("items");

    assert!(cctxs.is_some());
    assert!(cctxs.unwrap().is_array());

    let cctxs = cctxs.unwrap().as_array().unwrap();

    assert_eq!(cctxs.len(), 3);


    let response: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/CctxInfo:list?limit=10&offset=0&status_reduced=Success,Pending",
    )
    .await;

    // The response should be a valid JSON object with a "cctxs" array
    assert!(response.is_object());

    let cctxs_response:ListCctxsResponse = serde_json::from_value(response).unwrap();

    assert_eq!(cctxs_response.items.len(), 5);


    
}

#[tokio::test]
#[ignore = "Needs database to run"]
async  fn test_list_cctxs_with_filters() {
    //if TEST_TRACING is true, then initi
    if std::env::var("TEST_TRACING").is_ok() {
        init_logs("test_list_cctxs_with_filters",
         &TracingSettings::default(),
          &JaegerSettings::default()).unwrap();
    }

    // Test the ListCctxs endpoint with filters
    let status_1 = "PendingInbound";
    let status_2 = "OutboundMined";
    let sender_address = "0x73B37B8BAbAC0e846bB2C4c581e60bFF2BFBE76e";
    let receiver_address = "0xa4dc1ebdcca3351f8d356910e7f17594c17f1747";
    let asset = "0x0000000000000000000000000000000000000000";
    let coin_type = "Zeta";
    let source_chain_ids = vec!["7001","7002"];
    let target_chain_id_1 = "97";
    let target_chain_id_2 = "96";
    let start_timestamp = "1752859110";
    let end_timestamp = "1752859111";

    let db = crate::helpers::init_db("test", "list_cctxs_with_filters").await;
    let db_url = db.db_url();

    let client = Client::new(RpcSettings::default());
    let base =
        crate::helpers::init_zetachain_cctx_server(db_url, |mut x| {
            x.tracing.enabled = false;
            x.indexer.enabled = false;
            x.websocket.enabled = false;
            x
        }, db.client(), Arc::new(client))
        .await;



    let mut cctx_1 = crate::helpers::dummy_cross_chain_tx("test_list_cctxs_with_filters_1", status_1);
    cctx_1.inbound_params.sender = sender_address.to_string();
    
    cctx_1.inbound_params.asset = asset.to_string();
    cctx_1.inbound_params.coin_type = CoinType::try_from(coin_type.to_string()).unwrap();
    cctx_1.inbound_params.sender_chain_id = source_chain_ids[0].to_string();
    cctx_1.outbound_params[0].receiver = receiver_address.to_string();
    cctx_1.outbound_params[0].receiver_chain_id = target_chain_id_1.to_string();
    cctx_1.cctx_status.status = status_1.to_string();
    cctx_1.cctx_status.created_timestamp = start_timestamp.to_string();
    cctx_1.cctx_status.last_update_timestamp = end_timestamp.to_string();

    let mut cctx_2 = crate::helpers::dummy_cross_chain_tx("test_list_cctxs_with_filters_2", status_2);
    cctx_2.inbound_params.sender = sender_address.to_string();
    cctx_2.inbound_params.asset = asset.to_string();
    cctx_2.inbound_params.coin_type = CoinType::try_from(coin_type.to_string()).unwrap();
    cctx_2.inbound_params.sender_chain_id = source_chain_ids[1].to_string();
    cctx_2.outbound_params[0].receiver = receiver_address.to_string();
    cctx_2.outbound_params[0].receiver_chain_id = target_chain_id_2.to_string();
    cctx_2.cctx_status.status = status_1.to_string();
    cctx_2.cctx_status.created_timestamp = start_timestamp.to_string();
    cctx_2.cctx_status.last_update_timestamp = end_timestamp.to_string();

    let database = ZetachainCctxDatabase::new(db.client());
    let tx = db.client().begin().await.unwrap();
    database.batch_insert_transactions(Uuid::new_v4(), &vec![cctx_1, cctx_2], &tx).await.unwrap();
    tx.commit().await.unwrap();

    let source_chain_id = source_chain_ids[0].to_string();
    let target_chain_id = format!("{},{}", target_chain_id_1, target_chain_id_2);
    let status_reduced = "Pending";
    let response: serde_json::Value = test_server::send_get_request(
        &base,
        format!("/api/v1/CctxInfo:list?limit=10&offset=0&status_reduced={status_reduced}&sender_address={sender_address}&receiver_address={receiver_address}&asset={asset}&coin_type={coin_type}&source_chain_id={source_chain_id}&target_chain_id={target_chain_id}&start_timestamp={start_timestamp}&end_timestamp={end_timestamp}").as_str(),
    )
    .await;

    // The response should be a valid JSON object with a "cctxs" array
    assert!(response.is_object());
    let cctxs = response.get("items");

    assert!(cctxs.is_some());
    assert!(cctxs.unwrap().is_array());

    let cctxs = cctxs.unwrap().as_array().unwrap();

    assert_eq!(cctxs.len(), 1);

    let status = "Success,Pending";
    let response: serde_json::Value = test_server::send_get_request(
        &base,
        format!("/api/v1/CctxInfo:list?limit=10&offset=0&status_reduced={status}").as_str(),
    )
    .await;

    let cctxs = response.get("items");

    let cctxs = cctxs.unwrap().as_array().unwrap();
    assert_eq!(cctxs.len(), 2);


}   

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_list_cctxs_with_status_reduced_filter() {
    let db = crate::helpers::init_db("test", "list_cctxs_with_status_reduced_filter").await;
    let db_url = db.db_url();

    let client = Client::new(RpcSettings::default());
    let base =
        crate::helpers::init_zetachain_cctx_server(db_url, |mut x| {
            x.tracing.enabled = false;
            x.indexer.enabled = false;
            x.websocket.enabled = false;
            x
        }, db.client(), Arc::new(client))
        .await;

    // Create CCTXs with different statuses that should map to the same reduced status
    let pending_cctxs: Vec<CrossChainTx> = vec![
        "test_status_reduced_pending_1",
        "test_status_reduced_pending_2",
        "test_status_reduced_pending_3"
    ]
    .iter()
    .map(|x| crate::helpers::dummy_cross_chain_tx(x, "PendingInbound"))
    .chain(vec![
        "test_status_reduced_pending_4",
        "test_status_reduced_pending_5"
    ]
    .iter()
    .map(|x| crate::helpers::dummy_cross_chain_tx(x, "PendingOutbound")))
    .chain(vec![
        "test_status_reduced_pending_6"
    ]
    .iter()
    .map(|x| crate::helpers::dummy_cross_chain_tx(x, "PendingRevert")))
    .collect();

    let success_cctxs: Vec<CrossChainTx> = vec![
        "test_status_reduced_success_1",
        "test_status_reduced_success_2"
    ]
    .iter()
    .map(|x| crate::helpers::dummy_cross_chain_tx(x, "OutboundMined"))
    .collect();

    let failed_cctxs: Vec<CrossChainTx> = vec![
        "test_status_reduced_failed_1",
        "test_status_reduced_failed_2"
    ]
    .iter()
    .map(|x| crate::helpers::dummy_cross_chain_tx(x, "Aborted"))
    .chain(vec![
        "test_status_reduced_failed_3"
    ]
    .iter()
    .map(|x| crate::helpers::dummy_cross_chain_tx(x, "Reverted")))
    .collect();

    let all_cctxs: Vec<CrossChainTx> = pending_cctxs
        .into_iter()
        .chain(success_cctxs.into_iter())
        .chain(failed_cctxs.into_iter())
        .collect();

    let database = ZetachainCctxDatabase::new(db.client());
    let tx = db.client().begin().await.unwrap();
    database.batch_insert_transactions(Uuid::new_v4(), &all_cctxs, &tx).await.unwrap();
    tx.commit().await.unwrap();

    // Test filtering by reduced status "Pending"
    let response: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/CctxInfo:list?limit=20&offset=0&status_reduced=Pending",
    )
    .await;

    assert!(response.is_object());
    let cctxs = response.get("items");
    assert!(cctxs.is_some());
    assert!(cctxs.unwrap().is_array());
    let cctxs = cctxs.unwrap().as_array().unwrap();
    assert_eq!(cctxs.len(), 6); // Should get all 6 pending CCTXs (PendingInbound, PendingOutbound, PendingRevert)

    // Test filtering by reduced status "Success"
    let response: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/CctxInfo:list?limit=20&offset=0&status_reduced=Success",
    )
    .await;

    assert!(response.is_object());
    let cctxs = response.get("items");
    assert!(cctxs.is_some());
    assert!(cctxs.unwrap().is_array());
    let cctxs = cctxs.unwrap().as_array().unwrap();
    assert_eq!(cctxs.len(), 2); // Should get all 2 success CCTXs (OutboundMined)

    // Test filtering by reduced status "Failed"
    let response: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/CctxInfo:list?limit=20&offset=0&status_reduced=Failed",
    )
    .await;

    assert!(response.is_object());
    let cctxs = response.get("items");
    assert!(cctxs.is_some());
    assert!(cctxs.unwrap().is_array());
    let cctxs = cctxs.unwrap().as_array().unwrap();
    assert_eq!(cctxs.len(), 3); // Should get all 3 failed CCTXs (Aborted, Reverted)
}