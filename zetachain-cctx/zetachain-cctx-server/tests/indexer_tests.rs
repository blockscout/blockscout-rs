use std::{sync::Arc, time::Duration};
mod helpers;

use actix_phoenix_channel::ChannelCentral;
use blockscout_service_launcher::test_server;
use pretty_assertions::assert_eq;
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use serde_json::json;
use uuid::Uuid;
use wiremock::{
    matchers::{method, path, path_regex, query_param},
    Mock, MockServer, ResponseTemplate,
};
use zetachain_cctx_entity::{
    cctx_status::{Column as CctxStatusColumn, Entity as CctxStatusEntity},
    cross_chain_tx,
    sea_orm_active_enums::{
        CctxStatusStatus::OutboundMined, CoinType, Kind, ProcessingStatus, ProtocolContractVersion,
    },
    watermark,
};
use zetachain_cctx_logic::{channel::Channel, database::Relation, models::CctxStatusStatus};

use crate::helpers::{
    dummy_cctx_with_pagination_response, dummy_cross_chain_tx, dummy_related_cctxs_response,
    empty_response,
};
use zetachain_cctx_logic::{
    client::{Client, RpcSettings},
    database::ZetachainCctxDatabase,
    indexer::Indexer,
    settings::IndexerSettings,
};
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::CrossChainTx as CrossChainTxProto;

#[tokio::test]
async fn test_status_update() {
    let db = crate::helpers::init_db("test", "indexer_status_update").await;

    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        helpers::init_tests_logs().await;
    }

    // Setup mock server
    let mock_server = MockServer::start().await;

    let pending_tx_index = "root_index";

    //import a single cctx
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "MH=="))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            dummy_cctx_with_pagination_response(&[pending_tx_index], "end"),
        ))
        .mount(&mock_server)
        .await;
    //supress further historical sync
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "end"))
        .respond_with(ResponseTemplate::new(200).set_body_json(empty_response()))
        .mount(&mock_server)
        .await;

    //supress realtime sync
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(empty_response()))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(
            r"/zeta-chain/crosschain/inboundHashToCctxData/\d+",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(
            {"CrossChainTxs": []}
        )))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            format!("/zeta-chain/crosschain/cctx/{pending_tx_index}").as_str(),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "CrossChainTx": dummy_cross_chain_tx(pending_tx_index, CctxStatusStatus::PendingOutbound)
        })))
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            format!("/zeta-chain/crosschain/cctx/{pending_tx_index}").as_str(),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "CrossChainTx": dummy_cross_chain_tx(pending_tx_index, CctxStatusStatus::OutboundMined)
        })))
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;

    let client = Client::new(RpcSettings {
        url: mock_server.uri().to_string(),
        request_per_second: 100,
        ..Default::default()
    });

    let database = ZetachainCctxDatabase::new(db.client().clone(), 7001);
    let channel = Arc::new(ChannelCentral::new(Channel));

    database.setup_db().await.unwrap();
    let indexer = Indexer::new(
        IndexerSettings {
            polling_interval: 100, // Fast polling for tests
            concurrency: 10,
            enabled: true,
            ..Default::default()
        },
        Arc::new(client),
        Arc::new(database),
        Arc::new(channel.channel_broadcaster()),
    );

    let indexer_handle = tokio::spawn(async move {
        let _ = indexer.run().await;
    });

    tokio::time::sleep(Duration::from_millis(1000)).await;

    indexer_handle.abort();

    let cctx_id = cross_chain_tx::Entity::find()
        .filter(cross_chain_tx::Column::Index.eq(pending_tx_index))
        .one(db.client().as_ref())
        .await
        .unwrap()
        .unwrap()
        .id;
    let cctx_status = CctxStatusEntity::find()
        .filter(CctxStatusColumn::CrossChainTxId.eq(cctx_id))
        .one(db.client().as_ref())
        .await
        .unwrap();

    assert!(cctx_status.is_some());
    assert_eq!(cctx_status.unwrap().status, OutboundMined);
}

#[tokio::test]
async fn test_status_update_links_related() {
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        helpers::init_tests_logs().await;
    }

    let db = crate::helpers::init_db("test", "indexer_status_update_links_related").await;
    let mock_server = MockServer::start().await;

    let root_index = "root_index";

    //This will make indexer import the root cctx
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "MH=="))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(dummy_cctx_with_pagination_response(&["root_index"], "end")),
        )
        .mount(&mock_server)
        .await;
    //This will just prevent historical sync from importing any more cctxs
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("pagination.key", "end"))
        .respond_with(ResponseTemplate::new(200).set_body_json(empty_response()))
        .mount(&mock_server)
        .await;
    //This will emulate a situation where the transaction come not in the original order
    //This will make realtime fetcher to import the child #3 before others
    //Because realtime fetcher has the highest priority
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            dummy_cctx_with_pagination_response(&["child_3_index"], "end"),
        ))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"/zeta-chain/crosschain/cctx/.+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "CrossChainTx": dummy_cross_chain_tx("root_index", CctxStatusStatus::OutboundMined)
        })))
        .mount(&mock_server)
        .await;

    let child_1_index = "child_1_index";

    //Check import a child cctx from inboundHashToCctxData
    Mock::given(method("GET"))
        .and(path(
            format!("/zeta-chain/crosschain/inboundHashToCctxData/{root_index}").as_str(),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(dummy_related_cctxs_response(&[child_1_index])),
        )
        .mount(&mock_server)
        .await;

    //Make the mock return a single child of depth 2
    let child_2_index = "child_2_index";
    Mock::given(method("GET"))
        .and(path(
            format!("/zeta-chain/crosschain/inboundHashToCctxData/{child_1_index}").as_str(),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(dummy_related_cctxs_response(&[child_2_index])),
        )
        .mount(&mock_server)
        .await;
    //This will let find out that we already have child_3_index in the database
    //And it will be updated by status update
    let child_3_index = "child_3_index";
    Mock::given(method("GET"))
        .and(path(
            format!("/zeta-chain/crosschain/inboundHashToCctxData/{child_2_index}",).as_str(),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(dummy_related_cctxs_response(&[child_3_index])),
        )
        .mount(&mock_server)
        .await;

    //This will let indexer know that there are no further children for child_3_index
    Mock::given(method("GET"))
        .and(path(
            format!("/zeta-chain/crosschain/inboundHashToCctxData/{child_3_index}",).as_str(),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(
            {"CrossChainTxs": []}
        )))
        .mount(&mock_server)
        .await;

    //insert starting point for historical sync
    watermark::Entity::insert(watermark::ActiveModel {
        id: ActiveValue::NotSet,
        kind: ActiveValue::Set(Kind::Historical),
        pointer: ActiveValue::Set("MH==".to_string()),
        processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
        created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_by: ActiveValue::Set("test".to_string()),
        ..Default::default()
    })
    .exec(db.client().as_ref())
    .await
    .unwrap();

    let client = Client::new(RpcSettings {
        url: mock_server.uri().to_string(),
        request_per_second: 100,
        ..Default::default()
    });

    let channel = Arc::new(ChannelCentral::new(Channel));

    let indexer = Indexer::new(
        IndexerSettings {
            polling_interval: 100, // Fast polling for tests
            concurrency: 10,
            retry_threshold: 3,
            enabled: true,
            ..Default::default()
        },
        Arc::new(client),
        Arc::new(ZetachainCctxDatabase::new(db.client().clone(), 7001)),
        Arc::new(channel.channel_broadcaster()),
    );

    let indexer_handle = tokio::spawn(async move {
        let _ = indexer.run().await;
    });

    tokio::time::sleep(Duration::from_millis(1000)).await;

    indexer_handle.abort();

    let root = cross_chain_tx::Entity::find()
        .filter(cross_chain_tx::Column::Index.eq(root_index))
        .one(db.client().as_ref())
        .await
        .unwrap();

    //root must be synced at this point by historical sync
    assert!(root.is_some());
    let root = root.unwrap();
    assert_eq!(root.depth, 0);
    assert_eq!(
        root.processing_status,
        ProcessingStatus::Done,
        "root cctx was not processed"
    );

    //child_1 must be synced at this point by status update
    let child_1 = cross_chain_tx::Entity::find()
        .filter(cross_chain_tx::Column::Index.eq(child_1_index))
        .one(db.client().as_ref())
        .await
        .unwrap();

    assert!(child_1.is_some());
    let child_1 = child_1.unwrap();
    assert_eq!(child_1.root_id, Some(root.id));
    assert_eq!(child_1.parent_id, Some(root.id));
    assert_eq!(child_1.depth, 1);

    //child_2 must be synced at this point by status update
    let child_2 = cross_chain_tx::Entity::find()
        .filter(cross_chain_tx::Column::Index.eq(child_2_index))
        .one(db.client().as_ref())
        .await
        .unwrap();

    assert!(child_2.is_some(), "discovered cctx was not imported");
    let child_2 = child_2.unwrap();
    assert_eq!(
        child_2.root_id,
        Some(root.id),
        "child root id has not been set"
    );
    assert_eq!(
        child_2.parent_id,
        Some(child_1.id),
        "child parent id has not been set"
    );
    assert_eq!(child_2.depth, 2, "child depth has not been set");

    let updated_child_3 = cross_chain_tx::Entity::find()
        .filter(cross_chain_tx::Column::Index.eq(child_3_index))
        .one(db.client().as_ref())
        .await
        .unwrap();

    assert!(
        updated_child_3.is_some(),
        "grandchild cctx was not imported by realtime fetcher"
    );
    let updated_child_3 = updated_child_3.unwrap();
    assert_eq!(
        updated_child_3.root_id,
        Some(root.id),
        "grandchild root id has not been set"
    );
    assert_eq!(
        updated_child_3.parent_id,
        Some(child_2.id),
        "grandchild parent id has not been set"
    );
    assert_eq!(
        updated_child_3.depth, 3,
        "grandchild depth has not been set"
    );
}

#[tokio::test]
async fn test_lock_watermark() {
    let db = crate::helpers::init_db("test", "indexer_lock_watermark").await;
    let database = ZetachainCctxDatabase::new(db.client().clone(), 7001);

    watermark::Entity::insert(watermark::ActiveModel {
        id: ActiveValue::NotSet,
        kind: ActiveValue::Set(Kind::Historical),
        pointer: ActiveValue::Set("MH==".to_string()),
        processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
        created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_by: ActiveValue::Set("test".to_string()),
        ..Default::default()
    })
    .exec(db.client().as_ref())
    .await
    .unwrap();

    let watermark = watermark::Entity::find()
        .filter(watermark::Column::Kind.eq(Kind::Historical))
        .one(db.client().as_ref())
        .await
        .unwrap();
    let watermark_id = watermark.clone().unwrap().id;
    database
        .lock_watermark(watermark_id, Uuid::new_v4())
        .await
        .unwrap();
    database
        .unlock_watermark(watermark_id, Uuid::new_v4())
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
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        helpers::init_tests_logs().await;
    }

    let db = crate::helpers::init_db("test", "indexer_get_cctx_info").await;
    let root_index = "root_cctx_index".to_string();
    let root_cctx = dummy_cross_chain_tx(&root_index, CctxStatusStatus::OutboundMined);
    let tx = db.begin().await.unwrap();
    let database = ZetachainCctxDatabase::new(db.client().clone(), 7001);
    database.setup_db().await.unwrap();
    database
        .batch_insert_transactions(Uuid::new_v4(), &vec![root_cctx], &tx, None)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let child_cctx_index = "child_cctx_index";
    let child_cctx = dummy_cross_chain_tx(child_cctx_index, CctxStatusStatus::OutboundMined);
    let tx = db.begin().await.unwrap();
    database
        .batch_insert_transactions(
            Uuid::new_v4(),
            &vec![child_cctx],
            &tx,
            Some(Relation {
                parent_id: 1,
                depth: 2,
                root_id: 1,
            }),
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let cctx = database
        .get_complete_cctx(root_index.clone())
        .await
        .unwrap();

    assert!(cctx.is_some());
    let cctx = cctx.unwrap();
    assert_eq!(cctx.index, root_index);
    assert_eq!(cctx.creator, "creator");
    assert_eq!(cctx.zeta_fees, "0");
    assert_eq!(cctx.relayed_message, "msg");
    assert_eq!(
        cctx.protocol_contract_version,
        i32::from(ProtocolContractVersion::V2)
    );
    assert_eq!(cctx.outbound_params.len(), 2);
    assert_eq!(cctx.outbound_params[0].receiver, "receiver");
    assert_eq!(cctx.outbound_params[0].receiver_chain_id, 2);
    assert_eq!(cctx.outbound_params[0].coin_type, i32::from(CoinType::Zeta));
    assert_eq!(cctx.outbound_params[0].amount, "1000000000000000000");
    assert_eq!(cctx.outbound_params[0].tss_nonce, 42);
    assert_eq!(cctx.outbound_params[0].gas_limit, 1337);

    let base = helpers::init_zetachain_cctx_server(
        db.db_url(),
        |mut x| {
            x.indexer.concurrency = 1;
            x.indexer.polling_interval = 1000;
            x.tracing.enabled = false;
            x.indexer.enabled = false;
            x.websocket.enabled = false;
            x
        },
        db.client(),
        Arc::new(Client::new(RpcSettings {
            ..Default::default()
        })),
    )
    .await;
    let response: serde_json::Value = test_server::send_get_request(
        &base,
        format!("/api/v1/CctxInfo:get?cctx_id={root_index}").as_str(),
    )
    .await;

    let parsed_cctx: CrossChainTxProto = serde_json::from_value(response).unwrap();

    assert_eq!(parsed_cctx.index, cctx.index);
    assert_eq!(parsed_cctx.creator, cctx.creator);
    assert_eq!(parsed_cctx.zeta_fees, cctx.zeta_fees);

    let related_cctxs = parsed_cctx.related_cctxs;
    let expected_coin_type: i32 =
        zetachain_cctx_proto::blockscout::zetachain_cctx::v1::CoinType::Erc20.into();
    assert_eq!(related_cctxs.len(), 2);
    assert_eq!(related_cctxs[1].index, child_cctx_index);
    assert_eq!(related_cctxs[1].depth, 2);
    assert_eq!(related_cctxs[1].source_chain_id, 1);
    assert_eq!(related_cctxs[1].status, 3);
    assert_eq!(related_cctxs[1].inbound_amount, "8504");
    assert_eq!(related_cctxs[1].inbound_coin_type, expected_coin_type);
    assert_eq!(related_cctxs[1].outbound_params.len(), 2);
    assert_eq!(related_cctxs[1].outbound_params[0].chain_id, 2);
    assert_eq!(related_cctxs[1].outbound_params[0].coin_type, 0);
    assert_eq!(
        related_cctxs[1].outbound_params[0].amount,
        "1000000000000000000"
    );
    assert_eq!(related_cctxs[1].outbound_params[1].chain_id, 3);
    assert_eq!(related_cctxs[1].outbound_params[1].coin_type, 2);
    assert_eq!(related_cctxs[1].outbound_params[1].amount, "42691234567890");
}
