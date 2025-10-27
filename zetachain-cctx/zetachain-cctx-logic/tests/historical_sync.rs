use std::{sync::Arc, time::Duration};
mod helpers;

use actix_phoenix_channel::ChannelCentral;
use pretty_assertions::assert_eq;
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::path_regex;

use wiremock::{
    matchers::{method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};
use zetachain_cctx_entity::{
    cctx_status::{Column as CctxStatusColumn, Entity as CctxStatusEntity},
    inbound_params, outbound_params, revert_options,
};

use zetachain_cctx_entity::{
    cctx_status, cross_chain_tx,
    sea_orm_active_enums::{Kind, ProcessingStatus},
    watermark,
};
use zetachain_cctx_logic::{
    channel::Channel,
    client::{Client, RpcSettings},
    database::ZetachainCctxDatabase,
    indexer::Indexer,
    models::CctxStatusStatus,
    settings::IndexerSettings,
};
#[tokio::test]
async fn test_historical_sync_updates_pointer() {
    //if env var TRACING=true then call init_tests_logs()
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        helpers::init_tests_logs().await;
    }
    let db = crate::helpers::init_db("test", "historical_sync_updates_pointer").await;

    // Setup mock server
    let mock_server = MockServer::start().await;

    // Mock responses for different pagination scenarios
    // Mock first page response (default case)
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "MH=="))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            helpers::dummy_cctx_with_pagination_response(
                &["page_1_index_1", "page_1_index_2"],
                "SECOND_PAGE",
            ),
        ))
        .mount(&mock_server)
        .await;

    // Mock second page response when pagination.key == "SECOND_PAGE"
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "SECOND_PAGE"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            helpers::dummy_cctx_with_pagination_response(
                &["page_2_index_1", "page_2_index_2"],
                "THIRD_PAGE",
            ),
        ))
        .mount(&mock_server)
        .await;

    // Mock third page response when pagination.key == "THIRD_PAGE"
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "THIRD_PAGE"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            helpers::dummy_cctx_with_pagination_response(
                &["page_3_index_1", "page_3_index_2"],
                "end",
            ),
        ))
        .mount(&mock_server)
        .await;

    // Mock third page response when pagination.key == "THIRD_PAGE"
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "end"))
        .respond_with(ResponseTemplate::new(200).set_body_json(helpers::empty_cctx_response()))
        .mount(&mock_server)
        .await;

    // Mock realtime fetch response (unordered=false)
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(helpers::empty_cctx_response()))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"/zeta-chain/crosschain/cctx/.+"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(helpers::dummy_cross_chain_tx(
                "dummy_index",
                CctxStatusStatus::OutboundMined,
            )),
        )
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(
            r"/zeta-chain/crosschain/inboundHashToCctxData/.+",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(
            {"CrossChainTxs": []}
        )))
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

    let database = ZetachainCctxDatabase::new(db_conn.clone(), 7001);
    database.setup_db().await.unwrap();

    let channel = Arc::new(ChannelCentral::new(Channel));
    // Create indexer
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

    // Run indexer for a short time to process historical data
    let indexer_handle = tokio::spawn(async move {
        // Run for a limited time to avoid infinite loop
        let timeout_duration = Duration::from_secs(1);
        tokio::time::timeout(timeout_duration, async {
            let _ = indexer.run().await;
        })
        .await
        .unwrap_or_else(|_| {
            tracing::info!("Indexer timed out");
        });
    });

    // Wait for indexer to process
    tokio::time::sleep(Duration::from_millis(5000)).await;

    // Cancel the indexer
    indexer_handle.abort();

    // Verify watermark was updated to the final pagination key
    let final_watermark = watermark::Entity::find()
        .filter(watermark::Column::Kind.eq(Kind::Historical))
        .one(db_conn.as_ref())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        final_watermark.pointer, "end",
        "watermark has not been properly updated"
    );

    // Verify that all CCTX records were inserted
    let cctx_count = cross_chain_tx::Entity::find()
        .count(db_conn.as_ref())
        .await
        .unwrap();

    // We expect 6 total CCTX records (2 from each page)
    assert_eq!(cctx_count, 6);

    // // Verify specific CCTX records exist
    for index in [
        "page_1_index_1",
        "page_1_index_2",
        "page_2_index_1",
        "page_2_index_2",
        "page_3_index_1",
        "page_3_index_2",
    ] {
        let cctx = cross_chain_tx::Entity::find()
            .filter(cross_chain_tx::Column::Index.eq(index))
            .one(db_conn.as_ref())
            .await
            .unwrap();
        assert!(cctx.is_some(), "cctx not found for index: {index}");
        let cctx = cctx.unwrap();
        let cctx_id = cctx.id;
        let inbound = inbound_params::Entity::find()
            .filter(inbound_params::Column::CrossChainTxId.eq(cctx_id))
            .one(db_conn.as_ref())
            .await
            .unwrap();
        assert!(
            inbound.is_some(),
            "inbound params not found for cctx: {index}"
        );
        let outbound = outbound_params::Entity::find()
            .filter(outbound_params::Column::CrossChainTxId.eq(cctx_id))
            .one(db_conn.as_ref())
            .await
            .unwrap();
        assert!(
            outbound.is_some(),
            "outbound params not found for cctx: {index}"
        );
        let status = CctxStatusEntity::find()
            .filter(CctxStatusColumn::CrossChainTxId.eq(cctx_id))
            .one(db_conn.as_ref())
            .await
            .unwrap();
        assert!(status.is_some(), "status not found for cctx: {index}");
        let revert = revert_options::Entity::find()
            .filter(revert_options::Column::CrossChainTxId.eq(cctx_id))
            .one(db_conn.as_ref())
            .await
            .unwrap();
        assert!(revert.is_some());
    }

    let cctxs_with_status = cross_chain_tx::Entity::find()
        .find_also_related(cctx_status::Entity)
        .filter(cross_chain_tx::Column::Index.is_in(vec![
            "page_1_index_1",
            "page_1_index_2",
            "page_2_index_1",
            "page_2_index_2",
            "page_3_index_1",
            "page_3_index_2",
        ]))
        .all(db_conn.as_ref())
        .await
        .unwrap();
    let max_timestamp = cctxs_with_status
        .iter()
        .map(|c| c.1.as_ref().unwrap().last_update_timestamp)
        .max()
        .unwrap();
    let watermark_timestamp = final_watermark.upper_bound_timestamp.unwrap();
    assert_eq!(max_timestamp, watermark_timestamp);
}

#[tokio::test]
async fn test_database_processes_one_off_watermark() {
    let db = crate::helpers::init_db("test", "test_database_processes_one_off_watermark").await;
    let db_conn = db.client();

    let database = ZetachainCctxDatabase::new(db_conn.clone(), 7001);

    database.setup_db().await.unwrap();

    let one_off_watermark = watermark::ActiveModel {
        id: ActiveValue::NotSet,
        kind: ActiveValue::Set(Kind::Historical),
        pointer: ActiveValue::Set("ONE_OFF".to_string()), // Start from beginning
        processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
        created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_by: ActiveValue::Set("test".to_string()),
        ..Default::default()
    };

    let one_off_watermark = watermark::Entity::insert(one_off_watermark)
        .exec_with_returning(db_conn.as_ref())
        .await
        .unwrap();

    let imported = database
        .import_cctxs(
            Uuid::new_v4(),
            vec![helpers::dummy_cross_chain_tx(
                "dummy_index",
                CctxStatusStatus::OutboundMined,
            )],
            "ONE_OFF",
            one_off_watermark.id,
            None,
        )
        .await
        .unwrap();

    assert_eq!(imported.len(), 1);
    assert_eq!(imported[0].index, "dummy_index");

    let watermark = watermark::Entity::find()
        .filter(watermark::Column::Id.eq(one_off_watermark.id))
        .one(db_conn.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(watermark.processing_status, ProcessingStatus::Done);
}
