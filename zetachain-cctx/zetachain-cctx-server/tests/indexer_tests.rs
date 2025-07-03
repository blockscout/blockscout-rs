use std::sync::Arc;
use std::time::Duration;
mod helpers;

use blockscout_service_launcher::test_server;
use chrono::{NaiveDateTime, Utc};
use pretty_assertions::assert_eq;
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use sea_orm::{ConnectionTrait, PaginatorTrait};
use serde_json::json;
use wiremock::matchers::path_regex;
use wiremock::{
    matchers::{method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};
use zetachain_cctx_entity::cctx_status::{Column as CctxStatusColumn, Entity as CctxStatusEntity};
use zetachain_cctx_entity::sea_orm_active_enums::CctxStatusStatus::OutboundMined;
use zetachain_cctx_entity::sea_orm_active_enums::{
    CoinType, ProcessingStatus, ProtocolContractVersion}
;
use zetachain_cctx_entity::{cross_chain_tx, sea_orm_active_enums::Kind, watermark};

use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::{CrossChainTx};
use zetachain_cctx_logic::{
    client::{Client, RpcSettings},
    database::ZetachainCctxDatabase,
    indexer::Indexer,
    settings::IndexerSettings,
};


#[tokio::test]
async fn test_historical_sync_updates_pointer() {
    let db = crate::helpers::init_db("test", "historical_sync_updates_pointer").await;

    // Setup mock server
    let mock_server = MockServer::start().await;

    // Mock responses for different pagination scenarios
    setup_historical_mock_responses(&mock_server).await;

    Mock::given(method("GET"))
        .and(path_regex(r"/crosschain/cctx/\d+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::from_str::<serde_json::Value>(&FINALIZED_TX_RESPONSE).unwrap(),
        ))
        .mount(&mock_server)
        .await;

    // Create client pointing to mock server
    let client = Client::new(RpcSettings {
        url: mock_server.uri().to_string(),
        request_per_second: 100,
        ..Default::default()
    });

    // Initialize database with historical watermark
    let db_conn = db.client();

    cross_chain_tx::Entity::delete_many()
        .exec(db_conn.as_ref())
        .await
        .unwrap();

    let watermark_model = watermark::ActiveModel {
        id: ActiveValue::NotSet,
        kind: ActiveValue::Set(Kind::Historical),
        pointer: ActiveValue::Set("MH==".to_string()), // Start from beginning
        processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
        created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
    };

    watermark::Entity::insert(watermark_model)
        .exec(db_conn.as_ref())
        .await
        .unwrap();

    // Create indexer
    let indexer = Indexer::new(
        IndexerSettings {
            polling_interval: 100, // Fast polling for tests
            concurrency: 10,
            ..Default::default()
        },
        db_conn.clone(),
        Arc::new(client),
        Arc::new(ZetachainCctxDatabase::new(db_conn.clone())),
    );

    
    // Run indexer for a short time to process historical data
    let indexer_handle = tokio::spawn(async move {
        // Run for a limited time to avoid infinite loop
        let timeout_duration = Duration::from_secs(5);
        tokio::time::timeout(timeout_duration, async {
            let _ = indexer.run().await;
        })
        .await
        .unwrap_or_else(|_| {
            // Timeout is expected as we want to stop after processing
        });
    });

    // Wait for indexer to process
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Cancel the indexer
    indexer_handle.abort();

    // Verify watermark was updated to the final pagination key
    let final_watermark = watermark::Entity::find()
        .filter(watermark::Column::Kind.eq(Kind::Historical))
        .one(db_conn.as_ref())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(final_watermark.pointer, "end");

    // Verify that all CCTX records were inserted
    let cctx_count = cross_chain_tx::Entity::find()
        .count(db_conn.as_ref())
        .await
        .unwrap();

    // We expect 9 total CCTX records (3 from each page)
    assert_eq!(cctx_count, 9);

    // // Verify specific CCTX records exist
    let first_page_cctx = cross_chain_tx::Entity::find()
        .filter(
            cross_chain_tx::Column::Index
                .eq("0x36b9bb2f1d745b41d8e64eb203752c02abe6f50c5e932563799d5cbf160f5117"),
        )
        .one(db_conn.as_ref())
        .await
        .unwrap();
    assert!(first_page_cctx.is_some());

    let second_page_cctx = cross_chain_tx::Entity::find()
        .filter(
            cross_chain_tx::Column::Index
                .eq("0xff18366b92db12f7204eca924bc8ffbf93f655c7a35b68ec38b38d61d49fbaeb"),
        )
        .one(db_conn.as_ref())
        .await
        .unwrap();
    assert!(second_page_cctx.is_some());

    let third_page_cctx = cross_chain_tx::Entity::find()
        .filter(
            cross_chain_tx::Column::Index
                .eq("0x7f70bf83ed66c8029d8b2fce9ca95a81d053243537d0ea694de5a9c8e7d42f31"),
        )
        .one(db_conn.as_ref())
        .await
        .unwrap();
    assert!(third_page_cctx.is_some());
}

#[tokio::test]
async fn test_status_update() {
    let db = crate::helpers::init_db("test", "indexer_status_update").await;

    // Setup mock server
    let mock_server = MockServer::start().await;

    let empty_response = serde_json::json!({
        "CrossChainTx": [],
        "pagination": {
            "next_key": "end",
            "total": "0"
        }
    });
    watermark::Entity::insert(watermark::ActiveModel {
        id: ActiveValue::NotSet,
        kind: ActiveValue::Set(Kind::Historical),
        pointer: ActiveValue::Set("MH==".to_string()),
        processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
        created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
    })
    .exec(db.client().as_ref())
    .await
    .unwrap();


    // blockscout_service_launcher::tracing::init_logs("test_status_update", &blockscout_service_launcher::tracing::TracingSettings{
    //     enabled: true,
    //     ..Default::default()
    // }, &blockscout_service_launcher::tracing::JaegerSettings::default()).unwrap();

    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "MH=="))
        .respond_with(ResponseTemplate::new(200)
        .set_body_json(serde_json::from_str::<serde_json::Value>(&PENDING_TX_PAGE).unwrap()))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "end"))
        .respond_with(ResponseTemplate::new(200)
        .set_body_json(empty_response))
        .mount(&mock_server)
        .await;



        Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .respond_with(ResponseTemplate::new(200)
        .set_body_json(serde_json::json!({
            "CrossChainTx": [],
            "pagination": {
                "next_key": "end",
                "total": "0"
            }
        })))
        .mount(&mock_server)
        .await;

    setup_status_update_mock_responses(&mock_server).await;

    


    let client = Client::new(RpcSettings {
        url: mock_server.uri().to_string(),
        request_per_second: 100,
        ..Default::default()
    });

    let indexer = Indexer::new(
        IndexerSettings {
            polling_interval: 100, // Fast polling for tests
            concurrency: 1,
            ..Default::default()
        },
        db.client().clone(),
        Arc::new(client),
        Arc::new(ZetachainCctxDatabase::new(db.client().clone())),
    );

    let indexer_handle = tokio::spawn(async move {
        let _ = indexer.run().await;
    });

    tokio::time::sleep(Duration::from_millis(3000)).await;

    indexer_handle.abort();

    let cctx = CctxStatusEntity::find()
        .filter(CctxStatusColumn::CrossChainTxId.eq(1))
        .one(db.client().as_ref())
        .await
        .unwrap();

    assert!(cctx.is_some());
    assert_eq!(cctx.unwrap().status, OutboundMined);
}

use blockscout_service_launcher::tracing::init_logs;
#[tokio::test]
async fn test_parse_historical_data() {
    let db = crate::helpers::init_db("test", "indexer_parse_historical_data").await;
    let mock_server = MockServer::start().await;
    let mock_response = historic_response_from_file();

    watermark::Entity::delete_many()
        .filter(watermark::Column::Kind.eq(Kind::Historical))
        .exec(db.client().as_ref())
        .await
        .unwrap();

    cross_chain_tx::Entity::delete_many()
        .exec(db.client().as_ref())
        .await
        .unwrap();

    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "MH=="))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::from_str::<serde_json::Value>(&mock_response).unwrap()),
        )
        .mount(&mock_server)
        .await;

    let empty_response = serde_json::json!({
        "CrossChainTx": [],
        "pagination": {
            "next_key": "end",
            "total": "0"
        }
    });

    Mock::given(method("GET"))
        .and(path_regex(r"/crosschain/cctx/\d+"))
        .respond_with(ResponseTemplate::new(200)
        .set_body_json(serde_json::from_str::<serde_json::Value>(&FINALIZED_TX_RESPONSE).unwrap()))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "end"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&empty_response))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::from_str::<serde_json::Value>(&empty_response.to_string()).unwrap(),
        ))
        .mount(&mock_server)
        .await;

    let client = Client::new(RpcSettings {
        url: mock_server.uri().to_string(),
        request_per_second: 100,
        ..Default::default()
    });

    //delete historical watermark
    watermark::Entity::delete_many()
        .filter(watermark::Column::Kind.eq(Kind::Historical))
        .exec(db.client().as_ref())
        .await
        .unwrap();

    cross_chain_tx::Entity::delete_many()
        .exec(db.client().as_ref())
        .await
        .unwrap();
    let indexer = Indexer::new(
        IndexerSettings {
            polling_interval: 500, // Fast polling for tests
            concurrency: 10,
            realtime_fetch_batch_size:1,
            status_update_batch_size: 1,
            ..Default::default()
        },
        db.client().clone(),
        Arc::new(client),
        Arc::new(ZetachainCctxDatabase::new(db.client().clone())),
    );


    // Run indexer for a short time to process historical data
    let indexer_handle = tokio::spawn(async move {
        let _ = indexer.run().await;
    });

    // Wait for indexer to process
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Cancel the indexer
    indexer_handle.abort();

    let cctx_count = cross_chain_tx::Entity::find()
        .count(db.client().as_ref())
        .await
        .unwrap();

    // assert_eq!(cctx_count, 100);

    // let first_page_cctx = cross_chain_tx::Entity::find()
    //     .filter(
    //         cross_chain_tx::Column::Index
    //             .eq("0x003d98f1eaa33775e97aef378d6d0dea466187239e90b3ecb0f0fca96f8facbf"),
    //     )
    //     .one(db.client().as_ref())
    //     .await
    //     .unwrap();

    // assert!(first_page_cctx.is_some());

    // let second_page_cctx = cross_chain_tx::Entity::find()
    //     .filter(
    //         cross_chain_tx::Column::Index
    //             .eq("0x003d98f1eaa33775e97aef378d6d0dea466187239e90b3ecb0f0fca96f8facbf"),
    //     )
    //     .one(db.client().as_ref())
    //     .await
    //     .unwrap();
    // assert!(second_page_cctx.is_some());
}

#[tokio::test]
async fn test_lock_watermark() {
    let db = crate::helpers::init_db("test", "indexer_lock_watermark").await;
    let database = ZetachainCctxDatabase::new(db.client().clone());

    watermark::Entity::insert(watermark::ActiveModel {
        id: ActiveValue::NotSet,
        kind: ActiveValue::Set(Kind::Historical),
        pointer: ActiveValue::Set("MH==".to_string()),
        processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
        created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
    })
    .exec(db.client().as_ref())
    .await
    .unwrap();

    let watermark = watermark::Entity::find()
        .filter(watermark::Column::Kind.eq(Kind::Historical))
        .one(db.client().as_ref())
        .await
        .unwrap();
    database
        .lock_watermark(watermark.clone().unwrap())
        .await
        .unwrap();
    database
        .unlock_watermark(watermark.clone().unwrap())
        .await
        .unwrap();

    let watermark = watermark::Entity::find()
        .filter(watermark::Column::Kind.eq(Kind::Historical))
        .one(db.client().as_ref())
        .await
        .unwrap();
    assert_eq!(
        watermark.unwrap().processing_status,
        ProcessingStatus::Unlocked
    );
}

#[tokio::test]
async fn test_get_cctx_info() {
    let db = crate::helpers::init_db("test", "indexer_get_cctx_info").await;
    let last_update_timestamp =
        chrono::DateTime::<Utc>::from_timestamp(1750344684 as i64, 0).unwrap();
    let insert_statement = format!(
        r#"
    INSERT INTO cross_chain_tx (id, creator, index, zeta_fees, processing_status, relayed_message, last_status_update_timestamp, protocol_contract_version) 
    VALUES (1, 'zeta18pksjzclks34qkqyaahf2rakss80mnusju77cm', '0x7f70bf83ed66c8029d8b2fce9ca95a81d053243537d0ea694de5a9c8e7d42f31', '0', 'unlocked'::processing_status, '', '2025-01-19 12:31:24', 'V2');
    INSERT INTO cctx_status (id, cross_chain_tx_id, status, status_message, error_message, last_update_timestamp, is_abort_refunded, created_timestamp, error_message_revert, error_message_abort) 
    VALUES (1, 1, 'OutboundMined', '', '', '{}', false, 1750344684, '', '');
    INSERT INTO inbound_params (id, cross_chain_tx_id, sender, sender_chain_id, tx_origin, coin_type, asset, amount, observed_hash, observed_external_height, ballot_index, finalized_zeta_height, tx_finalization_status, is_cross_chain_call, status, confirmation_mode) 
    VALUES (1, 1, 'tb1q99jmq3q5s9hzm65vg0lyk3eqht63ssw8uyzy67', '18333', 'tb1q99jmq3q5s9hzm65vg0lyk3eqht63ssw8uyzy67', 'Gas', '', '8504', 'ed64294c274f4c8204f9c8e8495495b839c59d3944bfcf51ec8ad6d3d5721e38', '257082', '0x7f70bf83ed66c8029d8b2fce9ca95a81d053243537d0ea694de5a9c8e7d42f31', '10966764', 'Executed', false, 'SUCCESS', 'SAFE');
    INSERT INTO outbound_params (id, cross_chain_tx_id, receiver, receiver_chain_id, coin_type, amount, tss_nonce, gas_limit, gas_price, gas_priority_fee, hash, ballot_index, observed_external_height, gas_used, effective_gas_price, effective_gas_limit, tss_pubkey, tx_finalization_status, call_options_gas_limit, call_options_is_arbitrary_call, confirmation_mode) 
    VALUES (1, 1, '0x33c2f2B93798629f1311cA9ade3D4BF732011718', '7001', 'Gas', '0', '0', '0', '', '', '0xcd6c1391ca9950bb527b36b21ac75bccd29e7a2adf105662cb5eadd4bda5b4d5', '', '10966764', '0', '0', '0', 'zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p', 'Executed', '0', false, 'SAFE');
    INSERT INTO revert_options (id, cross_chain_tx_id, revert_address, call_on_revert, abort_address, revert_message, revert_gas_limit) 
    VALUES (1, 1, '', false, '', NULL, '0');
    "#,
        last_update_timestamp.naive_utc().to_string()
    );

    // let insert_statement = Statement::from_string(db.client().as_ref().get_database_backend(), insert_statement);

    db.client()
        .execute_unprepared(&insert_statement)
        .await
        .unwrap();

    let database = ZetachainCctxDatabase::new(db.client().clone());

    let cctx = database
        .get_complete_cctx(
            "0x7f70bf83ed66c8029d8b2fce9ca95a81d053243537d0ea694de5a9c8e7d42f31".to_string(),
        )
        .await
        .unwrap();

    let mock_server = MockServer::start().await;

    let empty_response = serde_json::json!({
        "CrossChainTx": [],
        "pagination": {
            "next_key": "end",
            "total": "0"
        }
    });
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&empty_response))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"/crosschain/cctx/\d+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::from_str::<serde_json::Value>(&FINALIZED_TX_RESPONSE).unwrap(),
        ))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"/inboundHashToCctxData/\d+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(
            {"CrossChainTxs": []}
        )))
        .mount(&mock_server)
        .await;

    assert!(cctx.is_some());
    let cctx = cctx.unwrap();
    assert_eq!(
        cctx.cctx.index,
        "0x7f70bf83ed66c8029d8b2fce9ca95a81d053243537d0ea694de5a9c8e7d42f31"
    );
    assert_eq!(
        cctx.cctx.creator,
        "zeta18pksjzclks34qkqyaahf2rakss80mnusju77cm"
    );
    assert_eq!(cctx.cctx.zeta_fees, "0");
    assert_eq!(cctx.cctx.relayed_message, Some("".to_string()));
    assert_eq!(
        cctx.cctx.protocol_contract_version,
        ProtocolContractVersion::V2
    );
    assert_eq!(
        cctx.cctx.last_status_update_timestamp,
        NaiveDateTime::parse_from_str("2025-01-19 12:31:24", "%Y-%m-%d %H:%M:%S").unwrap()
    );
    assert_eq!(cctx.outbounds.len(), 1);
    assert_eq!(
        cctx.outbounds[0].receiver,
        "0x33c2f2B93798629f1311cA9ade3D4BF732011718"
    );
    assert_eq!(cctx.outbounds[0].receiver_chain_id, "7001");
    assert_eq!(cctx.outbounds[0].coin_type, CoinType::Gas);
    assert_eq!(cctx.outbounds[0].amount, "0");
    assert_eq!(cctx.outbounds[0].tss_nonce, "0");
    assert_eq!(cctx.outbounds[0].gas_limit, "0");

    let client = Client::new(RpcSettings {
        request_per_second: 100,
        url: mock_server.uri().to_string(),
        ..Default::default()
    });
    let base = helpers::init_zetachain_cctx_server(
        db.db_url(),
        |mut x| {
            x.indexer.concurrency = 1;
            x.indexer.polling_interval = 1000;
            // x.tracing.enabled = false;
            x
        },
        db.client(),
        Arc::new(client),
    )
    .await;
    let response: serde_json::Value = test_server::send_get_request(&base, "/api/v1/CctxInfoService:get?cctx_id=0x7f70bf83ed66c8029d8b2fce9ca95a81d053243537d0ea694de5a9c8e7d42f31")
                .await;
    let expected_response = json!({
        "creator": "zeta18pksjzclks34qkqyaahf2rakss80mnusju77cm",
        "index": "0x7f70bf83ed66c8029d8b2fce9ca95a81d053243537d0ea694de5a9c8e7d42f31",
        "zeta_fees": "0",
        "relayed_message": "",
        "cctx_status": {
            "status": "OUTBOUND_MINED",
            "status_message": "",
            "error_message": "",
            "last_update_timestamp": "1750344684",
            "is_abort_refunded": false,
            "created_timestamp": "1750344684",
            "error_message_revert": "",
            "error_message_abort": ""
        },
        "inbound_params": {
            "sender": "tb1q99jmq3q5s9hzm65vg0lyk3eqht63ssw8uyzy67",
            "sender_chain_id": "18333",
            "tx_origin": "tb1q99jmq3q5s9hzm65vg0lyk3eqht63ssw8uyzy67",
            "coin_type": "GAS",
            "asset": "",
            "amount": "8504",
            "observed_hash": "ed64294c274f4c8204f9c8e8495495b839c59d3944bfcf51ec8ad6d3d5721e38",
            "observed_external_height": "257082",
            "ballot_index": "0x7f70bf83ed66c8029d8b2fce9ca95a81d053243537d0ea694de5a9c8e7d42f31",
            "finalized_zeta_height": "10966764",
            "tx_finalization_status": "EXECUTED",
            "is_cross_chain_call": false,
            "status": "SUCCESS",
            "confirmation_mode": "SAFE"
        },
        "outbound_params": [
            {
                "receiver": "0x33c2f2B93798629f1311cA9ade3D4BF732011718",
                "receiver_chain_id": "7001",
                "coin_type": "GAS",
                "amount": "0",
                "tss_nonce": "0",
                "gas_limit": "0",
                "gas_price": "",
                "gas_priority_fee": "",
                "hash": "0xcd6c1391ca9950bb527b36b21ac75bccd29e7a2adf105662cb5eadd4bda5b4d5",
                "ballot_index": "",
                "observed_external_height": "10966764",
                "gas_used": "0",
                "effective_gas_price": "0",
                "effective_gas_limit": "0",
                "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                "tx_finalization_status": "EXECUTED",
                "call_options": {
                    "gas_limit": "0",
                    "is_arbitrary_call": false
                },
                "confirmation_mode": "SAFE"
            }
        ],
        "protocol_contract_version": "V2",
        "revert_options": {
            "revert_address": "",
            "call_on_revert": false,
            "abort_address": "",
            "revert_message": "",
            "revert_gas_limit": "0"
        }
    });

    let parsed_cctx: CrossChainTx = serde_json::from_value(response).unwrap();
    let expected_cctx: CrossChainTx = serde_json::from_value(expected_response).unwrap();

    assert_eq!(parsed_cctx.index, expected_cctx.index);
    assert_eq!(parsed_cctx.creator, expected_cctx.creator);
    assert_eq!(parsed_cctx.zeta_fees, expected_cctx.zeta_fees);
    assert_eq!(parsed_cctx.relayed_message, expected_cctx.relayed_message);
    assert!(parsed_cctx.cctx_status.is_some());
    let expected_cctx_status = expected_cctx.cctx_status.unwrap();
    let parsed_cctx_status = parsed_cctx.cctx_status.unwrap();
    assert_eq!(parsed_cctx_status.status, expected_cctx_status.status);
    assert_eq!(
        parsed_cctx_status.status_message,
        expected_cctx_status.status_message
    );
    assert_eq!(
        parsed_cctx_status.error_message,
        expected_cctx_status.error_message
    );
    assert_eq!(
        parsed_cctx_status.last_update_timestamp,
        expected_cctx_status.last_update_timestamp
    );
    assert_eq!(
        parsed_cctx_status.is_abort_refunded,
        expected_cctx_status.is_abort_refunded
    );
    assert_eq!(
        parsed_cctx_status.created_timestamp,
        expected_cctx_status.created_timestamp
    );
}

#[tokio::test]
async fn test_gap_fill() {
    let db = crate::helpers::init_db("test", "indexer_gap_fill").await;

    //simulate some sync progress
    // let last_update_timestamp= chrono::DateTime::<Utc>::from_timestamp(1750344684 as i64, 0).unwrap();
    let insert_statement = format!(
        r#"
    INSERT INTO cross_chain_tx (creator, index, zeta_fees, processing_status, relayed_message, last_status_update_timestamp, protocol_contract_version,tree_query_flag) 
    VALUES ('zeta18pksjzclks34qkqyaahf2rakss80mnusju77cm', '0x7f70bf83ed66c8029d8b2fce9ca95a81d053243537d0ea694de5a9c8e7d42f31', '0', 'unlocked'::processing_status, '', '2025-01-19 12:31:24', 'V2',true);
    INSERT INTO cctx_status (cross_chain_tx_id, status, status_message, error_message, last_update_timestamp, is_abort_refunded, created_timestamp, error_message_revert, error_message_abort) 
    VALUES (1, 'OutboundMined', '', '', '2025-01-19 12:31:24', false, 1750344684, '', '');
    "#
    );

    db.client()
        .execute_unprepared(&insert_statement)
        .await
        .unwrap();

    // suppress historical sync
    watermark::Entity::insert(watermark::ActiveModel {
        id: ActiveValue::NotSet,
        kind: ActiveValue::Set(Kind::Historical),
        pointer: ActiveValue::Set("end".to_string()),
        processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
        created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
    })
    .exec(db.client().as_ref())
    .await
    .unwrap();

    let mock_server = MockServer::start().await;
    let empty_response = serde_json::json!({
        "CrossChainTx": [],
        "pagination": {
            "next_key": "end",
            "total": "0"
        }
    });
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "end"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::from_str::<serde_json::Value>(&empty_response.to_string()).unwrap(),
        ))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .and(query_param("pagination.key", "SECOND_PAGE"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::from_str::<serde_json::Value>(SECOND_PAGE_RESPONSE).unwrap(),
        ))
        .mount(&mock_server)
        .await;

    // simulate realtime fetcher getting some relevant data
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                serde_json::from_str::<serde_json::Value>(FIRST_PAGE_RESPONSE).unwrap(),
            ),
        )
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"/inboundHashToCctxData/\d+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(
            {"CrossChainTxs": []}
        )))
        .mount(&mock_server)
        .await;

    // since these cctx are absent, the indexer is exprected to follow through and request the next page

    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .and(query_param("pagination.key", "THIRD_PAGE"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                serde_json::from_str::<serde_json::Value>(THIRD_PAGE_RESPONSE).unwrap(),
            ),
        )
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;

    let rpc_client = Client::new(RpcSettings {
        url: mock_server.uri(),
        request_per_second: 100,
        ..Default::default()
    });

    let indexer = Indexer::new(
        IndexerSettings {
            polling_interval: 100, // Fast polling for tests
            concurrency: 1,
            ..Default::default()
        },
        db.client().clone(),
        Arc::new(rpc_client),
        Arc::new(ZetachainCctxDatabase::new(db.client().clone())),
    );

    let indexer_handle = tokio::spawn(async move {
        let _ = indexer.run().await;
    });

    tokio::time::sleep(Duration::from_millis(2000)).await;

    indexer_handle.abort();

    let cctx_count = cross_chain_tx::Entity::find()
        .count(db.client().as_ref())
        .await
        .unwrap();

    // 7 cctxs are expected: 1 historical, 6 realtime
    assert_eq!(cctx_count, 7);
}

async fn setup_status_update_mock_responses(mock_server: &MockServer) {
    let pending_tx_index = "0xb313d88712a40bcc30b4b7c9aa6f073b9f9eb6e2ae3e4d6e704bd9c15c8a7759";
    Mock::given(method("GET"))
        .and(path(
            format!("/crosschain/cctx/{}", pending_tx_index).as_str(),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                serde_json::from_str::<serde_json::Value>(PENDING_TX_RESPONSE).unwrap(),
            ),
        )
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            format!("/crosschain/cctx/{}", pending_tx_index).as_str(),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::from_str::<serde_json::Value>(FINALIZED_TX_RESPONSE).unwrap(),
        ))
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;
}
async fn setup_historical_mock_responses(mock_server: &MockServer) {
    // Mock first page response (default case)
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "MH=="))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                serde_json::from_str::<serde_json::Value>(FIRST_PAGE_RESPONSE).unwrap(),
            ),
        )
        .mount(mock_server)
        .await;

    // Mock second page response when pagination.key == "SECOND_PAGE"
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "SECOND_PAGE"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::from_str::<serde_json::Value>(SECOND_PAGE_RESPONSE).unwrap(),
        ))
        .mount(mock_server)
        .await;

    // Mock third page response when pagination.key == "THIRD_PAGE"
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "THIRD_PAGE"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                serde_json::from_str::<serde_json::Value>(THIRD_PAGE_RESPONSE).unwrap(),
            ),
        )
        .mount(mock_server)
        .await;

    let empty_response = serde_json::json!({
        "CrossChainTx": [],
        "pagination": {
            "next_key": "end",
            "total": "0"
        }
    });

    // Mock third page response when pagination.key == "THIRD_PAGE"
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "end"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::from_str::<serde_json::Value>(&empty_response.to_string()).unwrap(),
        ))
        .mount(mock_server)
        .await;

    // Mock realtime fetch response (unordered=false)
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(empty_response))
        .mount(mock_server)
        .await;
}

use std::fs;

pub const FIRST_PAGE_RESPONSE: &str = r#"
{
    "CrossChainTx": [
        {
            "creator": "",
            "index": "0x36b9bb2f1d745b41d8e64eb203752c02abe6f50c5e932563799d5cbf160f5117",
            "zeta_fees": "10959181044563",
            "relayed_message": "",
            "cctx_status": {
                "status": "OutboundMined",
                "status_message": "",
                "error_message": "",
                "lastUpdate_timestamp": "1750344444",
                "isAbortRefunded": false,
                "created_timestamp": "1750344018",
                "error_message_revert": "",
                "error_message_abort": ""
            },
            "inbound_params": {
                "sender": "0x73B37B8BAbAC0e846bB2C4c581e60bFF2BFBE76e",
                "sender_chain_id": "7001",
                "tx_origin": "0x07Db16a4c71DD0467994e7f0dC121E7D96d36c8A",
                "coin_type": "Zeta",
                "asset": "",
                "amount": "100000000000000000",
                "observed_hash": "0xcbe2d7febc3dcebae09af25a84d138f095d4a6f8c949cd80276c663c1b62a6e6",
                "observed_external_height": "10966606",
                "ballot_index": "0x36b9bb2f1d745b41d8e64eb203752c02abe6f50c5e932563799d5cbf160f5117",
                "finalized_zeta_height": "0",
                "tx_finalization_status": "NotFinalized",
                "is_cross_chain_call": false,
                "status": "SUCCESS",
                "confirmation_mode": "SAFE"
            },
            "outbound_params": [
                {
                    "receiver": "0x07db16a4c71dd0467994e7f0dc121e7d96d36c8a",
                    "receiver_chainId": "11155111",
                    "coin_type": "Zeta",
                    "amount": "99989040818955437",
                    "tss_nonce": "2881",
                    "gas_limit": "0",
                    "gas_price": "2400616",
                    "gas_priority_fee": "0",
                    "hash": "0xb80b0b22b5392d8d5ef04273e73fefb7f94b25a99e66dee3317b6071e49acf5a",
                    "ballot_index": "0x27245f0d719c8bfcb9192dad814b8550a0d0191426935b1d93a55fdd6b081801",
                    "observed_external_height": "8583496",
                    "gas_used": "48129",
                    "effective_gas_price": "2400616",
                    "effective_gas_limit": "100000",
                    "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                    "tx_finalization_status": "Executed",
                    "call_options": {
                        "gas_limit": "90000",
                        "is_arbitrary_call": false
                    },
                    "confirmation_mode": "SAFE"
                }
            ],
            "protocol_contract_version": "V1",
            "revert_options": {
                "revert_address": "",
                "call_on_revert": false,
                "abort_address": "",
                "revert_message": null,
                "revert_gas_limit": "0"
            }
        },
        {
            "creator": "",
            "index": "0x3c61b9bb605136895bec5c6adacaabb85f08a3692d486d7d1849f5e8010eb74b",
            "zeta_fees": "0",
            "relayed_message": "33633663646232653832383065613663326337623539386662323037623536653464363830653963343635326435323163383538343465613365306535373465653330656635336131633232623363306362353937306134653133336133383132323733",
            "cctx_status": {
                "status": "OutboundMined",
                "status_message": "",
                "error_message": "",
                "lastUpdate_timestamp": "1750344144",
                "isAbortRefunded": false,
                "created_timestamp": "1750343826",
                "error_message_revert": "",
                "error_message_abort": ""
            },
            "inbound_params": {
                "sender": "0x5e55d01aC8677f70ddd437d647b3548456871123",
                "sender_chain_id": "7001",
                "tx_origin": "0x5e55d01aC8677f70ddd437d647b3548456871123",
                "coin_type": "ERC20",
                "asset": "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238",
                "amount": "1000",
                "observed_hash": "0xa6ccbbc42159f92949cef8304a0ef6f39d11ba127038a29cd639fb8ef68716e8",
                "observed_external_height": "10966560",
                "ballot_index": "0x3c61b9bb605136895bec5c6adacaabb85f08a3692d486d7d1849f5e8010eb74b",
                "finalized_zeta_height": "0",
                "tx_finalization_status": "NotFinalized",
                "is_cross_chain_call": true,
                "status": "SUCCESS",
                "confirmation_mode": "SAFE"
            },
            "outbound_params": [
                {
                    "receiver": "0x65e9Bd75449DFE94fF7a627e81084E37f3E5C422",
                    "receiver_chainId": "11155111",
                    "coin_type": "ERC20",
                    "amount": "1000",
                    "tss_nonce": "2880",
                    "gas_limit": "0",
                    "gas_price": "1200183",
                    "gas_priority_fee": "0",
                    "hash": "0x18ea3bafce2f25fbcbb337c749993b4014328cc809b64a8f5dfb1f80c1f8b692",
                    "ballot_index": "0x8a471ed21254eccb72ad933cc609bb100339e6902bfa9fc1140c27955e1fee20",
                    "observed_external_height": "8583473",
                    "gas_used": "172123",
                    "effective_gas_price": "1200183",
                    "effective_gas_limit": "300000",
                    "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                    "tx_finalization_status": "Executed",
                    "call_options": {
                        "gas_limit": "300000",
                        "is_arbitrary_call": false
                    },
                    "confirmation_mode": "SAFE"
                }
            ],
            "protocol_contract_version": "V2",
            "revert_options": {
                "revert_address": "0x0000000000000000000000000000000000000000",
                "call_on_revert": false,
                "abort_address": "0x0000000000000000000000000000000000000000",
                "revert_message": null,
                "revert_gas_limit": "0"
            }
        },
        {
            "creator": "zeta1dxyzsket66vt886ap0gnzlnu5pv0y99v086wnz",
            "index": "0xa43b4573e40da5c36e06ab01b24a3b3e2c5cf223760471d76177b681cf625638",
            "zeta_fees": "0",
            "relayed_message": "61343332613062373065656332366466333730393764353964316135646439333765646366393333323965333365623665636436383035623831363831633261636663396438656563636635343262633639326139313837613666346639396331653463",
            "cctx_status": {
                "status": "OutboundMined",
                "status_message": "",
                "error_message": "",
                "lastUpdate_timestamp": "1750343808",
                "isAbortRefunded": false,
                "created_timestamp": "1750343808",
                "error_message_revert": "",
                "error_message_abort": ""
            },
            "inbound_params": {
                "sender": "0x5e55d01aC8677f70ddd437d647b3548456871123",
                "sender_chain_id": "11155111",
                "tx_origin": "0x5e55d01aC8677f70ddd437d647b3548456871123",
                "coin_type": "ERC20",
                "asset": "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238",
                "amount": "1000",
                "observed_hash": "0x698d62baa3ae209207f6a74a3d4e51c3c73e7207f41f3a356f8459110004aafc",
                "observed_external_height": "8583445",
                "ballot_index": "0xa43b4573e40da5c36e06ab01b24a3b3e2c5cf223760471d76177b681cf625638",
                "finalized_zeta_height": "10966556",
                "tx_finalization_status": "Executed",
                "is_cross_chain_call": true,
                "status": "SUCCESS",
                "confirmation_mode": "SAFE"
            },
            "outbound_params": [
                {
                    "receiver": "0x9EfeB21fdFC99B640187C45155DC84ebEF47104f",
                    "receiver_chainId": "7001",
                    "coin_type": "ERC20",
                    "amount": "0",
                    "tss_nonce": "0",
                    "gas_limit": "0",
                    "gas_price": "",
                    "gas_priority_fee": "",
                    "hash": "0xf43eb574b701f0b6eb50f0e8b7638867fe5ef81608d20b84ea17548a8024a11b",
                    "ballot_index": "",
                    "observed_external_height": "10966556",
                    "gas_used": "0",
                    "effective_gas_price": "0",
                    "effective_gas_limit": "0",
                    "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                    "tx_finalization_status": "Executed",
                    "call_options": {
                        "gas_limit": "1500000",
                        "is_arbitrary_call": false
                    },
                    "confirmation_mode": "SAFE"
                }
            ],
            "protocol_contract_version": "V2",
            "revert_options": {
                "revert_address": "0x0000000000000000000000000000000000000000",
                "call_on_revert": false,
                "abort_address": "0x0000000000000000000000000000000000000000",
                "revert_message": null,
                "revert_gas_limit": "0"
            }
        }
    ],
    "pagination": {
        "next_key": "SECOND_PAGE",
        "total": "0"
    }
}
"#;

pub const SECOND_PAGE_RESPONSE: &str = r#"
{
    "CrossChainTx": [
        {
            "creator": "zeta1j8g8ch4uqgl3gtet3nntvczaeppmlxajqwh5u6",
            "index": "0xff18366b92db12f7204eca924bc8ffbf93f655c7a35b68ec38b38d61d49fbaeb",
            "zeta_fees": "0",
            "relayed_message": "000000000000000000000000f255835ca48cd5b2ca37771a158922e2575ebc490000000000000000000000007cb5e8e39cbf751cbcfbf8a4a80fc8ad04fdf01698dcea32f8e9b565d70d98dcca59b26a68d78cac896314ea993730640fe8e864",
            "cctx_status": {
                "status": "OutboundMined",
                "status_message": "",
                "error_message": "",
                "lastUpdate_timestamp": "1750350182",
                "isAbortRefunded": false,
                "created_timestamp": "1750350182",
                "error_message_revert": "",
                "error_message_abort": ""
            },
            "inbound_params": {
                "sender": "0x071D7eF713b3c3b4bc22b279E7cE3770F9B4f4B9",
                "sender_chain_id": "11155111",
                "tx_origin": "0x071D7eF713b3c3b4bc22b279E7cE3770F9B4f4B9",
                "coin_type": "NoAssetCall",
                "asset": "",
                "amount": "0",
                "observed_hash": "0xe8a1469f07e853a5e995668194ba456c1e52e827aefa8a92d3eead9c8c19b870",
                "observed_external_height": "8583972",
                "ballot_index": "0xff18366b92db12f7204eca924bc8ffbf93f655c7a35b68ec38b38d61d49fbaeb",
                "finalized_zeta_height": "10968070",
                "tx_finalization_status": "Executed",
                "is_cross_chain_call": false,
                "status": "SUCCESS",
                "confirmation_mode": "SAFE"
            },
            "outbound_params": [
                {
                    "receiver": "0xD8d6f3ac69c0903A3fe8d2c0c1A2dE4DC989374b",
                    "receiver_chainId": "7001",
                    "coin_type": "NoAssetCall",
                    "amount": "0",
                    "tss_nonce": "0",
                    "gas_limit": "0",
                    "gas_price": "",
                    "gas_priority_fee": "",
                    "hash": "0x523f02f42ae65ed8ef56844bd56bb9ec06213679604a33eedaaa1465f6179a9c",
                    "ballot_index": "",
                    "observed_external_height": "10968070",
                    "gas_used": "0",
                    "effective_gas_price": "0",
                    "effective_gas_limit": "0",
                    "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                    "tx_finalization_status": "Executed",
                    "call_options": {
                        "gas_limit": "1500000",
                        "is_arbitrary_call": false
                    },
                    "confirmation_mode": "SAFE"
                }
            ],
            "protocol_contract_version": "V2",
            "revert_options": {
                "revert_address": "0x071D7eF713b3c3b4bc22b279E7cE3770F9B4f4B9",
                "call_on_revert": false,
                "abort_address": "0x0000000000000000000000000000000000000000",
                "revert_message": "AAAAAAAAAAAAAAAA8lWDXKSM1bLKN3caFYki4ldevEkAAAAAAAAAAAAAAAB8tejjnL91HLz7+KSoD8itBP3wFpjc6jL46bVl1w2Y3MpZsmpo14ysiWMU6pk3MGQP6Ohk",
                "revert_gas_limit": "0"
            }
        },
        {
            "creator": "zeta1j8g8ch4uqgl3gtet3nntvczaeppmlxajqwh5u6",
            "index": "0x60d7e3b6d06a7deb38217c317f707bfbbb4a87b03fa6813eaea27448f71e438d",
            "zeta_fees": "0",
            "relayed_message": "000000000000000000000000f33e7b4c86661214346f974b4f7a9bd086bc72d3000000000000000000000000b0a1c893e6f6dd165d8f77f157aac9caf489f74c98dcea32f8e9b565d70d98dcca59b26a68d78cac896314ea993730640fe8e864",
            "cctx_status": {
                "status": "OutboundMined",
                "status_message": "",
                "error_message": "",
                "lastUpdate_timestamp": "1750349701",
                "isAbortRefunded": false,
                "created_timestamp": "1750349701",
                "error_message_revert": "",
                "error_message_abort": ""
            },
            "inbound_params": {
                "sender": "0x2914c4b9079e4892DF38D2CD8f964f3E64422dC3",
                "sender_chain_id": "11155111",
                "tx_origin": "0x2914c4b9079e4892DF38D2CD8f964f3E64422dC3",
                "coin_type": "NoAssetCall",
                "asset": "",
                "amount": "0",
                "observed_hash": "0x4aadbde3292132cd26a9259860e90acd1063b4537918a58ae307909bf2a0b7f0",
                "observed_external_height": "8583933",
                "ballot_index": "0x60d7e3b6d06a7deb38217c317f707bfbbb4a87b03fa6813eaea27448f71e438d",
                "finalized_zeta_height": "10967956",
                "tx_finalization_status": "Executed",
                "is_cross_chain_call": false,
                "status": "SUCCESS",
                "confirmation_mode": "SAFE"
            },
            "outbound_params": [
                {
                    "receiver": "0xD8d6f3ac69c0903A3fe8d2c0c1A2dE4DC989374b",
                    "receiver_chainId": "7001",
                    "coin_type": "NoAssetCall",
                    "amount": "0",
                    "tss_nonce": "0",
                    "gas_limit": "0",
                    "gas_price": "",
                    "gas_priority_fee": "",
                    "hash": "0x6d9882defc163d9710ef677989051fa7e18ae45bd544f3eda1816b82e0af1458",
                    "ballot_index": "",
                    "observed_external_height": "10967956",
                    "gas_used": "0",
                    "effective_gas_price": "0",
                    "effective_gas_limit": "0",
                    "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                    "tx_finalization_status": "Executed",
                    "call_options": {
                        "gas_limit": "1500000",
                        "is_arbitrary_call": false
                    },
                    "confirmation_mode": "SAFE"
                }
            ],
            "protocol_contract_version": "V2",
            "revert_options": {
                "revert_address": "0x2914c4b9079e4892DF38D2CD8f964f3E64422dC3",
                "call_on_revert": false,
                "abort_address": "0x0000000000000000000000000000000000000000",
                "revert_message": "AAAAAAAAAAAAAAAA8z57TIZmEhQ0b5dLT3qb0Ia8ctMAAAAAAAAAAAAAAACwociT5vbdFl2Pd/FXqsnK9In3TJjc6jL46bVl1w2Y3MpZsmpo14ysiWMU6pk3MGQP6Ohk",
                "revert_gas_limit": "0"
            }
        },
        {
            "creator": "zeta1mte0r3jzkf2rkd7ex4p3xsd3fxqg7q29q0wxl5",
            "index": "0xf79685f9581b10914080dba78514c1fea3003fc6d68a5b92cb359d18d9b67407",
            "zeta_fees": "0",
            "relayed_message": "",
            "cctx_status": {
                "status": "OutboundMined",
                "status_message": "",
                "error_message": "",
                "lastUpdate_timestamp": "1750344953",
                "isAbortRefunded": false,
                "created_timestamp": "1750344953",
                "error_message_revert": "",
                "error_message_abort": ""
            },
            "inbound_params": {
                "sender": "0x71973ec13be525f912b1bcda5d631f10388cb13cd2202b7b8650d4d6fe4b2339",
                "sender_chain_id": "103",
                "tx_origin": "0x71973ec13be525f912b1bcda5d631f10388cb13cd2202b7b8650d4d6fe4b2339",
                "coin_type": "Gas",
                "asset": "",
                "amount": "1000000",
                "observed_hash": "9H1aPvzrUTTaXHGKVXesCecnWMRZaBGKjtgWeTk6hfXs",
                "observed_external_height": "209702944",
                "ballot_index": "0xf79685f9581b10914080dba78514c1fea3003fc6d68a5b92cb359d18d9b67407",
                "finalized_zeta_height": "10966828",
                "tx_finalization_status": "Executed",
                "is_cross_chain_call": false,
                "status": "SUCCESS",
                "confirmation_mode": "SAFE"
            },
            "outbound_params": [
                {
                    "receiver": "0x4955a3F38ff86ae92A914445099caa8eA2B9bA32",
                    "receiver_chainId": "7001",
                    "coin_type": "Gas",
                    "amount": "0",
                    "tss_nonce": "0",
                    "gas_limit": "0",
                    "gas_price": "",
                    "gas_priority_fee": "",
                    "hash": "0x4a3e77ab9536c74b8321afd1190492dfddc6eeff8f2fb108b7dcf7a0c37471b4",
                    "ballot_index": "",
                    "observed_external_height": "10966828",
                    "gas_used": "0",
                    "effective_gas_price": "0",
                    "effective_gas_limit": "0",
                    "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                    "tx_finalization_status": "Executed",
                    "call_options": {
                        "gas_limit": "1500000",
                        "is_arbitrary_call": false
                    },
                    "confirmation_mode": "SAFE"
                }
            ],
            "protocol_contract_version": "V2",
            "revert_options": {
                "revert_address": "",
                "call_on_revert": false,
                "abort_address": "",
                "revert_message": null,
                "revert_gas_limit": "0"
            }
        }
    ],
    "pagination": {
        "next_key": "THIRD_PAGE",
        "total": "0"
    }
}
"#;

pub const THIRD_PAGE_RESPONSE: &str = r#"
{
    "CrossChainTx": [
        {
            "creator": "zeta18pksjzclks34qkqyaahf2rakss80mnusju77cm",
            "index": "0x7f70bf83ed66c8029d8b2fce9ca95a81d053243537d0ea694de5a9c8e7d42f31",
            "zeta_fees": "0",
            "relayed_message": "",
            "cctx_status": {
                "status": "OutboundMined",
                "status_message": "",
                "error_message": "",
                "lastUpdate_timestamp": "1750344684",
                "isAbortRefunded": false,
                "created_timestamp": "1750344684",
                "error_message_revert": "",
                "error_message_abort": ""
            },
            "inbound_params": {
                "sender": "tb1q99jmq3q5s9hzm65vg0lyk3eqht63ssw8uyzy67",
                "sender_chain_id": "18333",
                "tx_origin": "tb1q99jmq3q5s9hzm65vg0lyk3eqht63ssw8uyzy67",
                "coin_type": "Gas",
                "asset": "",
                "amount": "8504",
                "observed_hash": "ed64294c274f4c8204f9c8e8495495b839c59d3944bfcf51ec8ad6d3d5721e38",
                "observed_external_height": "257082",
                "ballot_index": "0x7f70bf83ed66c8029d8b2fce9ca95a81d053243537d0ea694de5a9c8e7d42f31",
                "finalized_zeta_height": "10966764",
                "tx_finalization_status": "Executed",
                "is_cross_chain_call": false,
                "status": "SUCCESS",
                "confirmation_mode": "SAFE"
            },
            "outbound_params": [
                {
                    "receiver": "0x33c2f2B93798629f1311cA9ade3D4BF732011718",
                    "receiver_chainId": "7001",
                    "coin_type": "Gas",
                    "amount": "0",
                    "tss_nonce": "0",
                    "gas_limit": "0",
                    "gas_price": "",
                    "gas_priority_fee": "",
                    "hash": "0xcd6c1391ca9950bb527b36b21ac75bccd29e7a2adf105662cb5eadd4bda5b4d5",
                    "ballot_index": "",
                    "observed_external_height": "10966764",
                    "gas_used": "0",
                    "effective_gas_price": "0",
                    "effective_gas_limit": "0",
                    "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                    "tx_finalization_status": "Executed",
                    "call_options": {
                        "gas_limit": "0",
                        "is_arbitrary_call": false
                    },
                    "confirmation_mode": "SAFE"
                }
            ],
            "protocol_contract_version": "V2",
            "revert_options": {
                "revert_address": "",
                "call_on_revert": false,
                "abort_address": "",
                "revert_message": null,
                "revert_gas_limit": "0"
            }
        },
        {
            "creator": "",
            "index": "0xf7b98c51a222d1499001eaa98004d7ee6ebebbdc46597d2d03197e8b0e7e16b2",
            "zeta_fees": "0",
            "relayed_message": "",
            "cctx_status": {
                "status": "OutboundMined",
                "status_message": "",
                "error_message": "",
                "lastUpdate_timestamp": "1750344706",
                "isAbortRefunded": false,
                "created_timestamp": "1750344287",
                "error_message_revert": "",
                "error_message_abort": ""
            },
            "inbound_params": {
                "sender": "0x58A8Ba18c585C411B95Ba1e78962a2A3E1c6f52a",
                "sender_chain_id": "7001",
                "tx_origin": "0x58A8Ba18c585C411B95Ba1e78962a2A3E1c6f52a",
                "coin_type": "Gas",
                "asset": "",
                "amount": "900000",
                "observed_hash": "0xa6e706fb088fb81598697da4eb0efccb8a6e4714afca6e8237bacee543b2bf4a",
                "observed_external_height": "10966670",
                "ballot_index": "0xf7b98c51a222d1499001eaa98004d7ee6ebebbdc46597d2d03197e8b0e7e16b2",
                "finalized_zeta_height": "0",
                "tx_finalization_status": "NotFinalized",
                "is_cross_chain_call": false,
                "status": "SUCCESS",
                "confirmation_mode": "SAFE"
            },
            "outbound_params": [
                {
                    "receiver": "tb1qhamwhl4w4sdv4j6g5vsfntna2a80kvkunllytl",
                    "receiver_chainId": "18333",
                    "coin_type": "Gas",
                    "amount": "900000",
                    "tss_nonce": "130",
                    "gas_limit": "0",
                    "gas_price": "36",
                    "gas_priority_fee": "0",
                    "hash": "ee9f4697a40dc84de2b3410887358e830e66fbd12e44b4a75b2364ae6b1a2ee5",
                    "ballot_index": "0x7d03a4fca71bee0dcd0d2bbce2add42d9921cb3ace4ee7de7e2b18beb528cc28",
                    "observed_external_height": "257083",
                    "gas_used": "0",
                    "effective_gas_price": "0",
                    "effective_gas_limit": "0",
                    "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                    "tx_finalization_status": "Executed",
                    "call_options": {
                        "gas_limit": "100",
                        "is_arbitrary_call": true
                    },
                    "confirmation_mode": "SAFE"
                }
            ],
            "protocol_contract_version": "V2",
            "revert_options": {
                "revert_address": "0x0000000000000000000000000000000000000000",
                "call_on_revert": false,
                "abort_address": "0x0000000000000000000000000000000000000000",
                "revert_message": null,
                "revert_gas_limit": "0"
            }
        },
        {
            "creator": "zeta1mte0r3jzkf2rkd7ex4p3xsd3fxqg7q29q0wxl5",
            "index": "0xb313d88712a40bcc30b4b7c9aa6f073b9f9eb6e2ae3e4d6e704bd9c15c8a7759",
            "zeta_fees": "0",
            "relayed_message": "",
            "cctx_status": {
                "status": "PendingOutbound",
                "status_message": "",
                "error_message": "",
                "lastUpdate_timestamp": "1750344267",
                "isAbortRefunded": false,
                "created_timestamp": "1750344267",
                "error_message_revert": "",
                "error_message_abort": ""
            },
            "inbound_params": {
                "sender": "tb1qhamwhl4w4sdv4j6g5vsfntna2a80kvkunllytl",
                "sender_chain_id": "18333",
                "tx_origin": "tb1qhamwhl4w4sdv4j6g5vsfntna2a80kvkunllytl",
                "coin_type": "Gas",
                "asset": "",
                "amount": "998368",
                "observed_hash": "fb9ed1a4f8e3971543f9a598a7b47f70173f5a5f31e43eac2e2fe7911c92254b",
                "observed_external_height": "257081",
                "ballot_index": "0xb313d88712a40bcc30b4b7c9aa6f073b9f9eb6e2ae3e4d6e704bd9c15c8a7759",
                "finalized_zeta_height": "10966666",
                "tx_finalization_status": "Executed",
                "is_cross_chain_call": false,
                "status": "SUCCESS",
                "confirmation_mode": "SAFE"
            },
            "outbound_params": [
                {
                    "receiver": "0x58A8Ba18c585C411B95Ba1e78962a2A3E1c6f52a",
                    "receiver_chainId": "7001",
                    "coin_type": "Gas",
                    "amount": "0",
                    "tss_nonce": "0",
                    "gas_limit": "0",
                    "gas_price": "",
                    "gas_priority_fee": "",
                    "hash": "0x4b963c94801f5c81b66c3f0e4ceb38fd18832ea43080060a5de6a7b5863f48d8",
                    "ballot_index": "",
                    "observed_external_height": "10966666",
                    "gas_used": "0",
                    "effective_gas_price": "0",
                    "effective_gas_limit": "0",
                    "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                    "tx_finalization_status": "NotFinalized",
                    "call_options": {
                        "gas_limit": "0",
                        "is_arbitrary_call": false
                    },
                    "confirmation_mode": "SAFE"
                }
            ],
            "protocol_contract_version": "V2",
            "revert_options": {
                "revert_address": "",
                "call_on_revert": false,
                "abort_address": "",
                "revert_message": null,
                "revert_gas_limit": "0"
            }
        }
    ],
    "pagination": {
        "next_key": "end",
        "total": "0"
    }
}
"#;

pub const PENDING_TX_PAGE: &str = r#"
    {
        "CrossChainTx":
        [
            {
                "creator": "zeta1mte0r3jzkf2rkd7ex4p3xsd3fxqg7q29q0wxl5",
                "index": "0xb313d88712a40bcc30b4b7c9aa6f073b9f9eb6e2ae3e4d6e704bd9c15c8a7759",
                "zeta_fees": "0",
                "relayed_message": "",
                "cctx_status": {
                    "status": "PendingOutbound",
                    "status_message": "",
                    "error_message": "",
                    "lastUpdate_timestamp": "1750344267",
                    "isAbortRefunded": false,
                    "created_timestamp": "1750344267",
                    "error_message_revert": "",
                    "error_message_abort": ""
                },
                "inbound_params": {
                    "sender": "tb1qhamwhl4w4sdv4j6g5vsfntna2a80kvkunllytl",
                    "sender_chain_id": "18333",
                    "tx_origin": "tb1qhamwhl4w4sdv4j6g5vsfntna2a80kvkunllytl",
                    "coin_type": "Gas",
                    "asset": "",
                    "amount": "998368",
                    "observed_hash": "fb9ed1a4f8e3971543f9a598a7b47f70173f5a5f31e43eac2e2fe7911c92254b",
                    "observed_external_height": "257081",
                    "ballot_index": "0xb313d88712a40bcc30b4b7c9aa6f073b9f9eb6e2ae3e4d6e704bd9c15c8a7759",
                    "finalized_zeta_height": "10966666",
                    "tx_finalization_status": "Executed",
                    "is_cross_chain_call": false,
                    "status": "SUCCESS",
                    "confirmation_mode": "SAFE"
                },
                "outbound_params": [
                    {
                        "receiver": "0x58A8Ba18c585C411B95Ba1e78962a2A3E1c6f52a",
                        "receiver_chainId": "7001",
                        "coin_type": "Gas",
                        "amount": "0",
                        "tss_nonce": "0",
                        "gas_limit": "0",
                        "gas_price": "",
                        "gas_priority_fee": "",
                        "hash": "0x4b963c94801f5c81b66c3f0e4ceb38fd18832ea43080060a5de6a7b5863f48d8",
                        "ballot_index": "",
                        "observed_external_height": "10966666",
                        "gas_used": "0",
                        "effective_gas_price": "0",
                        "effective_gas_limit": "0",
                        "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                        "tx_finalization_status": "NotFinalized",
                        "call_options": {
                            "gas_limit": "0",
                            "is_arbitrary_call": false
                        },
                        "confirmation_mode": "SAFE"
                    }
                ],
                "protocol_contract_version": "V2",
                "revert_options": {
                    "revert_address": "",
                    "call_on_revert": false,
                    "abort_address": "",
                    "revert_message": null,
                    "revert_gas_limit": "0"
                }
            }
        ],
    "pagination": {
        "next_key": "end",
        "total": "0"
    }
}
"#;

pub const PENDING_TX_RESPONSE: &str = r#"
{"CrossChainTx": 
    {
            "creator": "zeta1mte0r3jzkf2rkd7ex4p3xsd3fxqg7q29q0wxl5",
            "index": "0xb313d88712a40bcc30b4b7c9aa6f073b9f9eb6e2ae3e4d6e704bd9c15c8a7759",
            "zeta_fees": "0",
            "relayed_message": "",
            "cctx_status": {
                "status": "PendingOutbound",
                "status_message": "",
                "error_message": "",
                "lastUpdate_timestamp": "1750344267",
                "isAbortRefunded": false,
                "created_timestamp": "1750344267",
                "error_message_revert": "",
                "error_message_abort": ""
            },
            "inbound_params": {
                "sender": "tb1qhamwhl4w4sdv4j6g5vsfntna2a80kvkunllytl",
                "sender_chain_id": "18333",
                "tx_origin": "tb1qhamwhl4w4sdv4j6g5vsfntna2a80kvkunllytl",
                "coin_type": "Gas",
                "asset": "",
                "amount": "998368",
                "observed_hash": "fb9ed1a4f8e3971543f9a598a7b47f70173f5a5f31e43eac2e2fe7911c92254b",
                "observed_external_height": "257081",
                "ballot_index": "0xb313d88712a40bcc30b4b7c9aa6f073b9f9eb6e2ae3e4d6e704bd9c15c8a7759",
                "finalized_zeta_height": "10966666",
                "tx_finalization_status": "Executed",
                "is_cross_chain_call": false,
                "status": "SUCCESS",
                "confirmation_mode": "SAFE"
            },
            "outbound_params": [
                {
                    "receiver": "0x58A8Ba18c585C411B95Ba1e78962a2A3E1c6f52a",
                    "receiver_chainId": "7001",
                    "coin_type": "Gas",
                    "amount": "0",
                    "tss_nonce": "0",
                    "gas_limit": "0",
                    "gas_price": "",
                    "gas_priority_fee": "",
                    "hash": "0x4b963c94801f5c81b66c3f0e4ceb38fd18832ea43080060a5de6a7b5863f48d8",
                    "ballot_index": "",
                    "observed_external_height": "10966666",
                    "gas_used": "0",
                    "effective_gas_price": "0",
                    "effective_gas_limit": "0",
                    "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                    "tx_finalization_status": "NotFinalized",
                    "call_options": {
                        "gas_limit": "0",
                        "is_arbitrary_call": false
                    },
                    "confirmation_mode": "SAFE"
                }
            ],
            "protocol_contract_version": "V2",
            "revert_options": {
                "revert_address": "",
                "call_on_revert": false,
                "abort_address": "",
                "revert_message": null,
                "revert_gas_limit": "0"
            }
        }
    }
"#;

pub const FINALIZED_TX_RESPONSE: &str = r#"
{
    "CrossChainTx": 
        {
            "creator": "zeta1mte0r3jzkf2rkd7ex4p3xsd3fxqg7q29q0wxl5",
            "index": "0xb313d88712a40bcc30b4b7c9aa6f073b9f9eb6e2ae3e4d6e704bd9c15c8a7759",
            "zeta_fees": "0",
            "relayed_message": "",
            "cctx_status": {
                "status": "OutboundMined",
                "status_message": "",
                "error_message": "",
                "lastUpdate_timestamp": "1750344267",
                "isAbortRefunded": false,
                "created_timestamp": "1750344267",
                "error_message_revert": "",
                "error_message_abort": ""
            },
            "inbound_params": {
                "sender": "tb1qhamwhl4w4sdv4j6g5vsfntna2a80kvkunllytl",
                "sender_chain_id": "18333",
                "tx_origin": "tb1qhamwhl4w4sdv4j6g5vsfntna2a80kvkunllytl",
                "coin_type": "Gas",
                "asset": "",
                "amount": "998368",
                "observed_hash": "fb9ed1a4f8e3971543f9a598a7b47f70173f5a5f31e43eac2e2fe7911c92254b",
                "observed_external_height": "257081",
                "ballot_index": "0xb313d88712a40bcc30b4b7c9aa6f073b9f9eb6e2ae3e4d6e704bd9c15c8a7759",
                "finalized_zeta_height": "10966666",
                "tx_finalization_status": "Executed",
                "is_cross_chain_call": false,
                "status": "SUCCESS",
                "confirmation_mode": "SAFE"
            },
            "outbound_params": [
                {
                    "receiver": "0x58A8Ba18c585C411B95Ba1e78962a2A3E1c6f52a",
                    "receiver_chainId": "7001",
                    "coin_type": "Gas",
                    "amount": "0",
                    "tss_nonce": "0",
                    "gas_limit": "0", 
                    "gas_price": "",
                    "gas_priority_fee": "",
                    "hash": "0x4b963c94801f5c81b66c3f0e4ceb38fd18832ea43080060a5de6a7b5863f48d8",
                    "ballot_index": "",
                    "observed_external_height": "10966666",
                    "gas_used": "0",
                    "effective_gas_price": "0",
                    "effective_gas_limit": "0",
                    "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                    "tx_finalization_status": "Executed",
                    "call_options": {
                        "gas_limit": "0",
                        "is_arbitrary_call": false
                    },
                    "confirmation_mode": "SAFE"
                }
            ],
            "protocol_contract_version": "V2",
            "revert_options": {
                "revert_address": "",
                "call_on_revert": false,
                "abort_address": "",
                "revert_message": null,
                "revert_gas_limit": "0"
            }
        }   
}
"#;

pub fn historic_response_from_file() -> String {
    let file_path = "tests/data/bad_response.json";
    let file_content = fs::read_to_string(file_path).expect("Failed to read file");
    file_content
}
