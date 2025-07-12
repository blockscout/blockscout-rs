use std::sync::Arc;
use std::time::Duration;
mod helpers;



use pretty_assertions::assert_eq;
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use sea_orm::PaginatorTrait;
use serde_json::json;
use wiremock::matchers::path_regex;
use wiremock::{
    matchers::{method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};
use zetachain_cctx_entity::cctx_status::{Column as CctxStatusColumn, Entity as CctxStatusEntity};
use zetachain_cctx_entity::{inbound_params, outbound_params, revert_options};

use zetachain_cctx_entity::sea_orm_active_enums::ProcessingStatus;
use zetachain_cctx_entity::{cross_chain_tx, sea_orm_active_enums::Kind, watermark};
use zetachain_cctx_logic::client::{Client, RpcSettings};
use zetachain_cctx_logic::database::ZetachainCctxDatabase;
use zetachain_cctx_logic::indexer::Indexer;
use zetachain_cctx_logic::settings::IndexerSettings;
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
        .and(path("/crosschain/cctx"))
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
        .and(path("/crosschain/cctx"))
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
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "THIRD_PAGE"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            helpers::dummy_cctx_with_pagination_response(&["page_3_index_1", "page_3_index_2"], "end"),
        ))
        .mount(&mock_server)
        .await;

    // Mock third page response when pagination.key == "THIRD_PAGE"
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "end"))
        .respond_with(ResponseTemplate::new(200).set_body_json(helpers::empty_cctx_response()))
        .mount(&mock_server)
        .await;

    // Mock realtime fetch response (unordered=false)
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(helpers::empty_cctx_response()))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"/crosschain/cctx/.+"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(helpers::dummy_cross_chain_tx("dummy_index", "OutboundMined")),
        )
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"/inboundHashToCctxData/.+"))
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


    let watermark_model = watermark::ActiveModel {
        id: ActiveValue::NotSet,
        kind: ActiveValue::Set(Kind::Historical),
        pointer: ActiveValue::Set("MH==".to_string()), // Start from beginning
        processing_status: ActiveValue::Set(ProcessingStatus::Unlocked),
        created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        updated_by: ActiveValue::Set("test".to_string()),
        ..Default::default()
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
            enabled: true,
            ..Default::default()
        },
        db_conn.clone(),
        Arc::new(client),
        Arc::new(ZetachainCctxDatabase::new(db_conn.clone())),
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
            // Timeout is expected as we want to stop after processing
        });
    });

    // Wait for indexer to process
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Cancel the indexer
    indexer_handle.abort();

    // Verify watermark was updated to the final pagination key
    let final_watermark = watermark::Entity::find()
        .filter(watermark::Column::Kind.eq(Kind::Historical))
        .one(db_conn.as_ref())
        .await
        .unwrap()
        .unwrap();

    println!(
        "final_watermark.pointer: {:?} ,updated_by: {:?}",
        final_watermark.pointer, final_watermark.updated_by
    );
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
    for index in ["page_1_index_1", "page_1_index_2", "page_2_index_1", "page_2_index_2", "page_3_index_1", "page_3_index_2"] {
        let cctx = cross_chain_tx::Entity::find()
            .filter(cross_chain_tx::Column::Index.eq(index))
            .one(db_conn.as_ref())
            .await
            .unwrap();
        assert!(cctx.is_some(), "cctx not found for index: {}", index);
        let cctx = cctx.unwrap();
        let cctx_id = cctx.id;
        let inbound = inbound_params::Entity::find()
            .filter(inbound_params::Column::CrossChainTxId.eq(cctx_id))
            .one(db_conn.as_ref())
            .await
            .unwrap();
        assert!(inbound.is_some(), "inbound params not found for cctx: {}", index);
        let outbound = outbound_params::Entity::find()
            .filter(outbound_params::Column::CrossChainTxId.eq(cctx_id))
            .one(db_conn.as_ref())
            .await
            .unwrap();
        assert!(outbound.is_some(), "outbound params not found for cctx: {}", index);
        let status = CctxStatusEntity::find()
            .filter(CctxStatusColumn::CrossChainTxId.eq(cctx_id))
            .one(db_conn.as_ref())
            .await
            .unwrap();
        assert!(status.is_some(), "status not found for cctx: {}", index);
        let revert = revert_options::Entity::find()
            .filter(revert_options::Column::CrossChainTxId.eq(cctx_id))
            .one(db_conn.as_ref())
            .await
            .unwrap();
        assert!(revert.is_some());
    }
}