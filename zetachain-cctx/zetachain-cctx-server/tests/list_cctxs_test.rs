mod helpers;
use blockscout_service_launcher::{
    test_server,
    tracing::{init_logs, JaegerSettings, TracingSettings},
};
use chrono::Utc;
use sea_orm::TransactionTrait;
use std::sync::Arc;
use uuid::Uuid;
use zetachain_cctx_logic::{
    client::{Client, RpcSettings},
    database::ZetachainCctxDatabase,
    models::{CctxStatusStatus, CoinType, CrossChainTx, Filters},
};
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::{
    CctxListItem, Direction, ListCctxsResponse,
};

use crate::helpers::{init_db, init_tests_logs};
// use crate::helpers::{init_db, init_zetachain_cctx_server};

#[tokio::test]
#[ignore = "Needs database to run"]
async fn list_cctx_sorting() {
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        init_tests_logs().await;
    }
    let db = init_db("test", "list_cctx_sorting").await;
    let database = ZetachainCctxDatabase::new(db.client(), 7001);
    let cctx_count: usize = 10;
    let limit: usize = 4;
    let cctxs: Vec<_> = (0..cctx_count)
        .map(|i| {
            let index_str = i.to_string();
            let mut cctx =
                helpers::dummy_cross_chain_tx(&index_str, CctxStatusStatus::OutboundMined);
            cctx.cctx_status.last_update_timestamp = index_str.clone();
            cctx.inbound_params.ballot_index = index_str.clone();
            cctx.inbound_params.observed_hash = index_str;
            cctx
        })
        .collect();

    let tx = db.client().begin().await.unwrap();
    database.setup_db().await.unwrap();
    database
        .batch_insert_transactions(Uuid::new_v4(), &cctxs, &tx, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let retrieved_desc = database
        .list_cctxs(limit as i64, None, Filters::default(), Direction::Desc)
        .await
        .unwrap();

    assert_eq!(retrieved_desc.items.len(), limit);
    assert_eq!(
        retrieved_desc.next_page_params.unwrap().page_key,
        (cctx_count - limit) as i64
    );

    let retrieved_asc = database
        .list_cctxs(limit as i64, None, Filters::default(), Direction::Asc)
        .await
        .unwrap();

    assert_eq!(retrieved_asc.items.len(), limit);
    assert_eq!(
        retrieved_asc.next_page_params.unwrap().page_key,
        (limit - 1) as i64
    );
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn list_cctx_timestamp_pagination() {
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        init_tests_logs().await;
    }
    let db = init_db("test", "list_cctx_timestamp_pagination").await;
    let database = ZetachainCctxDatabase::new(db.client(), 7001);
    let cctx_count: usize = 10;
    let limit: usize = 3;

    // Create a base timestamp and add incremental seconds to ensure distinct timestamps
    let base_timestamp = Utc::now().timestamp() - 1000; // Start 1000 seconds ago

    let cctxs: Vec<_> = (0..cctx_count)
        .map(|i| {
            let index_str = i.to_string();
            let mut cctx =
                helpers::dummy_cross_chain_tx(&index_str, CctxStatusStatus::OutboundMined);
            // Set last_update_timestamp to incremental values for predictable ordering
            let timestamp_epoch = base_timestamp + (i as i64 * 100); // 100 seconds apart
            cctx.cctx_status.last_update_timestamp = timestamp_epoch.to_string();
            cctx.inbound_params.ballot_index = index_str.clone();
            cctx.inbound_params.observed_hash = index_str;
            cctx
        })
        .collect();

    let tx = db.client().begin().await.unwrap();
    database.setup_db().await.unwrap();
    database
        .batch_insert_transactions(Uuid::new_v4(), &cctxs, &tx, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Test DESC pagination (most recent first)
    let first_page_desc = database
        .list_cctxs(limit as i64, None, Filters::default(), Direction::Desc)
        .await
        .unwrap();

    assert_eq!(first_page_desc.items.len(), limit);
    let first_page_timestamps: Vec<i64> = first_page_desc
        .items
        .iter()
        .map(|item| item.last_update_timestamp)
        .collect();

    // Verify DESC ordering (timestamps should be decreasing)
    for i in 1..first_page_timestamps.len() {
        assert!(
            first_page_timestamps[i - 1] > first_page_timestamps[i],
            "DESC ordering failed: {} should be > {}",
            first_page_timestamps[i - 1],
            first_page_timestamps[i]
        );
    }

    // Test pagination with page_key (using the minimum timestamp from first page)
    let page_key_desc = first_page_desc.next_page_params.as_ref().unwrap().page_key;

    let second_page_desc = database
        .list_cctxs(
            limit as i64,
            Some(page_key_desc),
            Filters::default(),
            Direction::Desc,
        )
        .await
        .unwrap();

    assert_eq!(second_page_desc.items.len(), limit);

    // Verify that all timestamps in second page are less than the page_key
    for item in &second_page_desc.items {
        assert!(
            item.last_update_timestamp < page_key_desc,
            "Timestamp {} should be < page_key {}",
            item.last_update_timestamp,
            page_key_desc
        );
    }

    // Test ASC pagination (oldest first)
    let first_page_asc = database
        .list_cctxs(limit as i64, None, Filters::default(), Direction::Asc)
        .await
        .unwrap();

    assert_eq!(first_page_asc.items.len(), limit);
    let first_page_timestamps_asc: Vec<i64> = first_page_asc
        .items
        .iter()
        .map(|item| item.last_update_timestamp)
        .collect();

    // Verify ASC ordering (timestamps should be increasing)
    for i in 1..first_page_timestamps_asc.len() {
        assert!(
            first_page_timestamps_asc[i - 1] < first_page_timestamps_asc[i],
            "ASC ordering failed: {} should be < {}",
            first_page_timestamps_asc[i - 1],
            first_page_timestamps_asc[i]
        );
    }

    // Test pagination with page_key for ASC
    let page_key_asc = first_page_asc.next_page_params.as_ref().unwrap().page_key;

    let second_page_asc = database
        .list_cctxs(
            limit as i64,
            Some(page_key_asc),
            Filters::default(),
            Direction::Asc,
        )
        .await
        .unwrap();

    assert_eq!(second_page_asc.items.len(), limit);

    // Verify that all timestamps in second page are greater than the page_key
    for item in &second_page_asc.items {
        assert!(
            item.last_update_timestamp > page_key_asc,
            "Timestamp {} should be > page_key {}",
            item.last_update_timestamp,
            page_key_asc
        );
    }

    // Test edge case: Using a timestamp that doesn't exist
    let non_existent_timestamp = base_timestamp + 50; // Between first two records
    let edge_case_result = database
        .list_cctxs(
            limit as i64,
            Some(non_existent_timestamp),
            Filters::default(),
            Direction::Desc,
        )
        .await
        .unwrap();

    // Should return records with timestamps less than the non-existent one
    for item in &edge_case_result.items {
        assert!(
            item.last_update_timestamp < non_existent_timestamp,
            "Timestamp {} should be < non_existent_timestamp {}",
            item.last_update_timestamp,
            non_existent_timestamp
        );
    }

    // Test edge case: Very old timestamp (should return all records for DESC)
    let very_old_timestamp = base_timestamp - 1000;
    let old_timestamp_result = database
        .list_cctxs(
            limit as i64,
            Some(very_old_timestamp),
            Filters::default(),
            Direction::Desc,
        )
        .await
        .unwrap();

    // Should return empty results since no records are older than this
    assert_eq!(old_timestamp_result.items.len(), 0);

    // Test edge case: Very future timestamp (should return all records for ASC)
    let future_timestamp = base_timestamp + 10000;
    let future_timestamp_result = database
        .list_cctxs(
            limit as i64,
            Some(future_timestamp),
            Filters::default(),
            Direction::Asc,
        )
        .await
        .unwrap();

    // Should return empty results since no records are newer than this
    assert_eq!(future_timestamp_result.items.len(), 0);
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn list_cctx_timestamp_conversion_edge_cases() {
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        init_tests_logs().await;
    }
    let db = init_db("test", "list_cctx_timestamp_conversion_edge_cases").await;
    let database = ZetachainCctxDatabase::new(db.client(), 7001);

    // Test with specific edge case timestamps that might cause conversion issues
    let edge_case_timestamps = [
        0i64,          // Unix epoch
        1i64,          // Very early timestamp
        946684800i64,  // Year 2000 timestamp
        1577836800i64, // Year 2020 timestamp
        2147483647i64, // Max 32-bit signed int (Year 2038 problem)
    ];

    let cctxs: Vec<_> = edge_case_timestamps
        .iter()
        .enumerate()
        .map(|(i, &timestamp)| {
            let index_str = format!("edge_case_{i}");
            let mut cctx =
                helpers::dummy_cross_chain_tx(&index_str, CctxStatusStatus::OutboundMined);
            cctx.cctx_status.last_update_timestamp = timestamp.to_string();
            cctx.inbound_params.ballot_index = index_str.clone();
            cctx.inbound_params.observed_hash = index_str;
            cctx
        })
        .collect();

    let tx = db.client().begin().await.unwrap();
    database.setup_db().await.unwrap();
    database
        .batch_insert_transactions(Uuid::new_v4(), &cctxs, &tx, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Test pagination with each edge case timestamp as page_key
    for (i, &test_timestamp) in edge_case_timestamps.iter().enumerate() {
        // Test DESC direction
        let result_desc = database
            .list_cctxs(
                10,
                Some(test_timestamp),
                Filters::default(),
                Direction::Desc,
            )
            .await;

        assert!(
            result_desc.is_ok(),
            "Failed to query with timestamp {} (index {}): {:?}",
            test_timestamp,
            i,
            result_desc.err()
        );

        let desc_items = result_desc.unwrap().items;
        for item in &desc_items {
            assert!(
                item.last_update_timestamp < test_timestamp,
                "DESC: Item timestamp {} should be < page_key {} (index {})",
                item.last_update_timestamp,
                test_timestamp,
                i
            );
        }

        // Test ASC direction
        let result_asc = database
            .list_cctxs(10, Some(test_timestamp), Filters::default(), Direction::Asc)
            .await;

        assert!(
            result_asc.is_ok(),
            "Failed to query with timestamp {} (index {}) in ASC: {:?}",
            test_timestamp,
            i,
            result_asc.err()
        );

        let asc_items = result_asc.unwrap().items;
        for item in &asc_items {
            assert!(
                item.last_update_timestamp > test_timestamp,
                "ASC: Item timestamp {} should be > page_key {} (index {})",
                item.last_update_timestamp,
                test_timestamp,
                i
            );
        }
    }

    // Test with negative timestamp (should fail gracefully or be handled)
    let negative_timestamp = -1i64;
    let negative_result = database
        .list_cctxs(
            10,
            Some(negative_timestamp),
            Filters::default(),
            Direction::Desc,
        )
        .await;

    // The query should either succeed with no results or fail gracefully
    // (depending on how DateTime::from_timestamp handles negative values)
    match negative_result {
        Ok(_response) => {
            // If it succeeds, it should return valid results (possibly empty)
            // This tests that the system handles negative timestamps gracefully
            // by either converting them properly or skipping invalid ones
        }
        Err(_) => {
            // If it fails, that's also acceptable behavior for invalid timestamps
            // This tests that the system handles edge cases gracefully
        }
    }

    // Test with very large timestamp (far future)
    let far_future_timestamp = 4102444800i64; // Year 2100
    let future_result = database
        .list_cctxs(
            10,
            Some(far_future_timestamp),
            Filters::default(),
            Direction::Desc,
        )
        .await
        .unwrap();

    // Should return all records since they're all before this future timestamp
    assert!(
        future_result.items.len() >= edge_case_timestamps.len(),
        "Future timestamp should return all records in DESC order"
    );

    // Verify that results are properly ordered by timestamp
    let timestamps: Vec<i64> = future_result
        .items
        .iter()
        .map(|item| item.last_update_timestamp)
        .collect();

    for i in 1..timestamps.len() {
        assert!(
            timestamps[i - 1] >= timestamps[i],
            "Timestamps should be in DESC order: {} >= {}",
            timestamps[i - 1],
            timestamps[i]
        );
    }
}

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

    let token = zetachain_cctx_logic::models::Token {
        name: "dummy_token_1".to_string(),
        symbol: "DUMMY".to_string(),
        asset: "0x0000000000000000000000000000000000000001".to_string(),
        foreign_chain_id: "7001".to_string(),
        coin_type: CoinType::ERC20,
        decimals: 18,
        gas_limit: "1000000000000000000".to_string(),
        paused: false,
        liquidity_cap: "1000000000000000000".to_string(),
        icon_url: None,
        zrc20_contract_address: Uuid::new_v4().to_string(),
    };

    let dummy_cctxs: Vec<CrossChainTx> = ["test_list_cctxs_endpoint_1"]
        .iter()
        .map(|x| {
            let mut cctx =
                crate::helpers::dummy_cross_chain_tx(x, CctxStatusStatus::PendingOutbound);
            cctx.inbound_params.asset = token.asset.clone();
            cctx.inbound_params.coin_type = token.coin_type.clone();
            cctx.inbound_params.sender_chain_id = token.foreign_chain_id.clone();
            cctx
        })
        .collect();

    let database = ZetachainCctxDatabase::new(db.client(), 7001);

    database.setup_db().await.unwrap();
    let tx = db.client().begin().await.unwrap();
    database
        .batch_insert_transactions(Uuid::new_v4(), &dummy_cctxs, &tx, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Test the ListCctxs endpoint
    let response: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/CctxInfo:list?limit=10&direction=DESC").await;

    // The response should be a valid JSON object with a "cctxs" array
    assert!(response.is_object());
    assert!(response.get("items").is_some());
    assert!(response.get("items").unwrap().is_array());
    let cctxs: Vec<CctxListItem> =
        serde_json::from_value(response.get("items").unwrap().clone()).unwrap();
    assert_eq!(cctxs.len(), 1);
    assert_eq!(cctxs[0].index, "test_list_cctxs_endpoint_1");
    assert_eq!(cctxs[0].status, 1);
    assert_eq!(cctxs[0].amount, "8504");
    assert_eq!(cctxs[0].source_chain_id, 7001);
    assert_eq!(cctxs[0].target_chain_id, 2);
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_list_cctxs_with_status_filter() {
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        init_logs(
            "test_list_cctxs_with_status_filter",
            &TracingSettings::default(),
            &JaegerSettings::default(),
        )
        .unwrap();
    }

    let db = crate::helpers::init_db("test", "list_cctxs_with_status_filter").await;
    let db_url = db.db_url();

    let client = Client::new(RpcSettings::default());
    let base = crate::helpers::init_zetachain_cctx_server(
        db_url,
        |mut x| {
            x.tracing.enabled = false;
            x.indexer.enabled = false;
            x.websocket.enabled = false;
            x
        },
        db.client(),
        Arc::new(client),
    )
    .await;
    let database = ZetachainCctxDatabase::new(db.client(), 7001);

    let token = zetachain_cctx_logic::models::Token {
        name: "dummy_token_1".to_string(),
        symbol: "DUMMY".to_string(),
        asset: "0x0000000000000000000000000000000000000001".to_string(),
        foreign_chain_id: "7001".to_string(),
        coin_type: CoinType::ERC20,
        decimals: 18,
        gas_limit: "1000000000000000000".to_string(),
        paused: false,
        liquidity_cap: "1000000000000000000".to_string(),
        icon_url: None,
        zrc20_contract_address: Uuid::new_v4().to_string(),
    };
    database.setup_db().await.unwrap();
    database
        .sync_tokens(Uuid::new_v4(), vec![token.clone()])
        .await
        .unwrap();
    let dummy_cctxs: Vec<CrossChainTx> = [
        "test_list_cctxs_with_status_filter_1",
        "test_list_cctxs_with_status_filter_2",
    ]
    .iter()
    .map(|x| crate::helpers::dummy_cross_chain_tx(x, CctxStatusStatus::OutboundMined))
    .chain(
        [
            "test_list_cctxs_with_status_filter_3",
            "test_list_cctxs_with_status_filter_4",
            "test_list_cctxs_with_status_filter_5",
        ]
        .iter()
        .map(|x| crate::helpers::dummy_cross_chain_tx(x, CctxStatusStatus::PendingOutbound)),
    )
    .map(|x| {
        let mut cctx = x;
        cctx.inbound_params.asset = token.asset.clone();
        cctx.inbound_params.coin_type = token.coin_type.clone();
        cctx.inbound_params.sender_chain_id = token.foreign_chain_id.clone();
        cctx
    })
    .collect();

    let tx = db.client().begin().await.unwrap();
    database
        .batch_insert_transactions(Uuid::new_v4(), &dummy_cctxs, &tx, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Test the ListCctxs endpoint with status filter
    let response: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/CctxInfo:list?limit=10&status_reduced=Success&direction=DESC",
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
        "/api/v1/CctxInfo:list?limit=10&status_reduced=Pending&direction=DESC",
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
        "/api/v1/CctxInfo:list?limit=10&status_reduced=Success,Pending&direction=DESC",
    )
    .await;

    // The response should be a valid JSON object with a "cctxs" array
    assert!(response.is_object());

    let cctxs_response: ListCctxsResponse = serde_json::from_value(response).unwrap();

    assert_eq!(cctxs_response.items.len(), 5);
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_list_cctxs_with_filters() {
    //if TEST_TRACING is true, then initi
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        init_logs(
            "test_list_cctxs_with_filters",
            &TracingSettings::default(),
            &JaegerSettings::default(),
        )
        .unwrap();
    }

    // Test the ListCctxs endpoint with filters
    let status_1 = CctxStatusStatus::PendingInbound;
    let status_2 = CctxStatusStatus::OutboundMined;
    let sender_address = "0x73B37B8BAbAC0e846bB2C4c581e60bFF2BFBE76e";
    let receiver_address = "0xa4dc1ebdcca3351f8d356910e7f17594c17f1747";
    let asset = "0x0000000000000000000000000000000000000000";
    let coin_type = CoinType::ERC20;
    let source_chain_ids = ["7001", "7002"];
    let target_chain_id_1 = "97";
    let target_chain_id_2 = "96";
    let start_timestamp = "1752859110";
    let end_timestamp = "1752859111";

    let db = crate::helpers::init_db("test", "list_cctxs_with_filters").await;
    let db_url = db.db_url();

    let client = Client::new(RpcSettings::default());
    let base = crate::helpers::init_zetachain_cctx_server(
        db_url,
        |mut x| {
            x.tracing.enabled = false;
            x.indexer.enabled = false;
            x.websocket.enabled = false;
            x
        },
        db.client(),
        Arc::new(client),
    )
    .await;

    let token = zetachain_cctx_logic::models::Token {
        name: "dummy_token_1".to_string(),
        symbol: "DUMMY".to_string(),
        asset: asset.to_string(),
        foreign_chain_id: "7001".to_string(),
        coin_type: coin_type.clone(),
        decimals: 18,
        gas_limit: "1000000000000000000".to_string(),
        paused: false,
        liquidity_cap: "1000000000000000000".to_string(),
        icon_url: None,
        zrc20_contract_address: Uuid::new_v4().to_string(),
    };

    let mut cctx_1 =
        crate::helpers::dummy_cross_chain_tx("test_list_cctxs_with_filters_1", status_1);
    cctx_1.inbound_params.asset = token.asset.clone();
    cctx_1.inbound_params.coin_type = token.coin_type.clone();
    cctx_1.inbound_params.sender_chain_id = token.foreign_chain_id.clone();
    cctx_1.inbound_params.sender = sender_address.to_string();

    cctx_1.outbound_params[0].receiver = receiver_address.to_string();
    cctx_1.outbound_params[0].receiver_chain_id = target_chain_id_1.to_string();
    cctx_1.cctx_status.created_timestamp = start_timestamp.to_string();
    cctx_1.cctx_status.last_update_timestamp = end_timestamp.to_string();

    let mut cctx_2 =
        crate::helpers::dummy_cross_chain_tx("test_list_cctxs_with_filters_2", status_2);
    cctx_2.inbound_params.asset = token.asset.clone();
    cctx_2.inbound_params.coin_type = token.coin_type.clone();
    cctx_2.inbound_params.sender_chain_id = token.foreign_chain_id.clone();
    cctx_2.inbound_params.sender = sender_address.to_string();
    cctx_2.outbound_params[0].receiver = receiver_address.to_string();
    cctx_2.outbound_params[0].receiver_chain_id = target_chain_id_2.to_string();
    cctx_2.cctx_status.created_timestamp = start_timestamp.to_string();
    cctx_2.cctx_status.last_update_timestamp = end_timestamp.to_string();

    let database = ZetachainCctxDatabase::new(db.client(), 7001);
    database.setup_db().await.unwrap();
    let tx = db.client().begin().await.unwrap();
    database
        .sync_tokens(Uuid::new_v4(), vec![token])
        .await
        .unwrap();
    database
        .batch_insert_transactions(Uuid::new_v4(), &vec![cctx_1, cctx_2], &tx, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let source_chain_id = source_chain_ids[0].to_string();
    let target_chain_id = format!("{target_chain_id_1},{target_chain_id_2}");
    let status_reduced = "Pending";
    let coin_type = coin_type.to_string();
    let mut path = "/api/v1/CctxInfo:list?".to_string();
    let filters = vec![
        ("limit", "10"),
        ("status_reduced", status_reduced),
        ("sender_address", sender_address),
        ("receiver_address", receiver_address),
        ("asset", asset),
        ("coin_type", &coin_type),
        ("source_chain_id", &source_chain_id),
        ("target_chain_id", &target_chain_id),
        ("start_timestamp", start_timestamp),
        ("end_timestamp", end_timestamp),
        ("direction", "DESC"),
    ];

    for (key, value) in filters {
        path.push_str(&format!("{key}={value}&"));
    }

    path.pop();

    let response: serde_json::Value = test_server::send_get_request(&base, path.as_str()).await;

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
        format!("/api/v1/CctxInfo:list?limit=10&status_reduced={status}&direction=DESC").as_str(),
    )
    .await;

    let cctxs = response.get("items");

    let cctxs = cctxs.unwrap().as_array().unwrap();
    assert_eq!(cctxs.len(), 2);
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_list_cctxs_with_status_reduced_filter() {
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        init_logs(
            "test_list_cctxs_with_status_reduced_filter",
            &TracingSettings::default(),
            &JaegerSettings::default(),
        )
        .unwrap();
    }
    let db = crate::helpers::init_db("test", "list_cctxs_with_status_reduced_filter").await;
    let db_url = db.db_url();

    let client = Client::new(RpcSettings::default());
    let base = crate::helpers::init_zetachain_cctx_server(
        db_url,
        |mut x| {
            x.tracing.enabled = false;
            x.indexer.enabled = false;
            x.websocket.enabled = false;
            x
        },
        db.client(),
        Arc::new(client),
    )
    .await;

    let token = zetachain_cctx_logic::models::Token {
        name: "dummy_token_1".to_string(),
        symbol: "DUMMY".to_string(),
        asset: "0x0000000000000000000000000000000000000001".to_string(),
        foreign_chain_id: "7001".to_string(),
        coin_type: CoinType::ERC20,
        decimals: 18,
        gas_limit: "1000000000000000000".to_string(),
        paused: false,
        liquidity_cap: "1000000000000000000".to_string(),
        icon_url: None,
        zrc20_contract_address: Uuid::new_v4().to_string(),
    };

    // Create CCTXs with different statuses that should map to the same reduced status
    let pending_cctxs: Vec<CrossChainTx> = [
        "test_status_reduced_pending_1",
        "test_status_reduced_pending_2",
        "test_status_reduced_pending_3",
    ]
    .iter()
    .map(|x| crate::helpers::dummy_cross_chain_tx(x, CctxStatusStatus::PendingInbound))
    .chain(
        [
            "test_status_reduced_pending_4",
            "test_status_reduced_pending_5",
        ]
        .iter()
        .map(|x| crate::helpers::dummy_cross_chain_tx(x, CctxStatusStatus::PendingOutbound)),
    )
    .chain(
        ["test_status_reduced_pending_6"]
            .iter()
            .map(|x| crate::helpers::dummy_cross_chain_tx(x, CctxStatusStatus::PendingRevert)),
    )
    .map(|x| {
        let mut cctx = x;
        cctx.inbound_params.asset = token.asset.clone();
        cctx.inbound_params.coin_type = token.coin_type.clone();
        cctx.inbound_params.sender_chain_id = token.foreign_chain_id.clone();
        cctx
    })
    .collect();

    let success_cctxs: Vec<CrossChainTx> = [
        "test_status_reduced_success_1",
        "test_status_reduced_success_2",
    ]
    .iter()
    .map(|x| crate::helpers::dummy_cross_chain_tx(x, CctxStatusStatus::OutboundMined))
    .collect();

    let aborted_cctxs: Vec<CrossChainTx> = [
        "test_status_reduced_failed_1",
        "test_status_reduced_failed_2",
    ]
    .iter()
    .map(|x| crate::helpers::dummy_cross_chain_tx(x, CctxStatusStatus::Aborted))
    .collect();
    let reverted_cctxs: CrossChainTx = crate::helpers::dummy_cross_chain_tx(
        "test_status_reduced_failed_3",
        CctxStatusStatus::Reverted,
    );

    let failed_cctxs: Vec<CrossChainTx> = aborted_cctxs
        .into_iter()
        .chain(vec![reverted_cctxs].into_iter())
        .collect();

    let all_cctxs: Vec<CrossChainTx> = pending_cctxs
        .into_iter()
        .chain(success_cctxs.into_iter())
        .chain(failed_cctxs.into_iter())
        .map(|x| {
            let mut cctx = x;
            cctx.inbound_params.asset = token.asset.clone();
            cctx.inbound_params.coin_type = token.coin_type.clone();
            cctx.inbound_params.sender_chain_id = token.foreign_chain_id.clone();
            cctx
        })
        .collect();

    let database = ZetachainCctxDatabase::new(db.client(), 7001);
    database.setup_db().await.unwrap();
    let tx = db.client().begin().await.unwrap();
    database
        .sync_tokens(Uuid::new_v4(), vec![token])
        .await
        .unwrap();
    database
        .batch_insert_transactions(Uuid::new_v4(), &all_cctxs, &tx, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Test filtering by reduced status "Pending"
    let response: serde_json::Value = test_server::send_get_request(
        &base,
        "/api/v1/CctxInfo:list?limit=20&status_reduced=Pending&direction=DESC",
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
        "/api/v1/CctxInfo:list?limit=20&status_reduced=Success&direction=DESC",
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
        "/api/v1/CctxInfo:list?limit=20&status_reduced=Failed&direction=DESC",
    )
    .await;

    assert!(response.is_object());
    let cctxs = response.get("items");
    assert!(cctxs.is_some());
    assert!(cctxs.unwrap().is_array());
    let cctxs = cctxs.unwrap().as_array().unwrap();
    assert_eq!(cctxs.len(), 3); // Should get all 3 failed CCTXs (Aborted, Reverted)
}
