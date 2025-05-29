use std::sync::Arc;
use std::time::Duration;
use pretty_assertions::assert_eq;
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use sea_orm::{ConnectionTrait, PaginatorTrait};
use serde_json::json;
use wiremock::matchers::path_regex;
use wiremock::{
    matchers::{method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};
use zetachain_cctx_entity::sea_orm_active_enums::ProcessingStatus;
use zetachain_cctx_entity::{cross_chain_tx, sea_orm_active_enums::Kind, watermark};

mod helpers;

use zetachain_cctx_logic::events::NoOpBroadcaster;
use zetachain_cctx_logic::{
    client::{Client, RpcSettings},
    database::ZetachainCctxDatabase,
    indexer::Indexer,
    settings::IndexerSettings,
};


#[tokio::test]
async fn test_level_data_gap() {
    
    let db = crate::helpers::init_db("test", "indexer_level_data_gap").await;

    //simulate some previous sync progress
    // let last_update_timestamp= chrono::DateTime::<Utc>::from_timestamp(1750344684 as i64, 0).unwrap();
    let insert_statement = format!(
        r#"
    INSERT INTO cross_chain_tx (creator, index, zeta_fees, processing_status, relayed_message, last_status_update_timestamp, protocol_contract_version,updated_by) 
    VALUES ('zeta18pksjzclks34qkqyaahf2rakss80mnusju77cm', '0x7f70bf83ed66c8029d8b2fce9ca95a81d053243537d0ea694de5a9c8e7d42f31', '0', 'Unlocked'::processing_status, '', '2025-01-19 12:31:24', 'V2','test');
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
        updated_by: ActiveValue::Set("test".to_string()),
        ..Default::default()
    })
    .exec(db.client().as_ref())
    .await
    .unwrap();

    let mock_server = MockServer::start().await;
    //suppress historical sync
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "end"))
        .respond_with(ResponseTemplate::new(200).set_body_json(helpers::empty_cctx_response()))
        .mount(&mock_server)
        .await;


        Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .and(query_param("pagination.key", "SECOND_PAGE"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            helpers::dummy_cctx_with_pagination_response(
                &["page_2_index_1", "page_2_index_2"],
                "THIRD_PAGE",
            ),
        ))
        .mount(&mock_server)
        .await;
    // since these cctx are absent, the indexer is exprected to follow through and request the next page

    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .and(query_param("pagination.key", "THIRD_PAGE"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            helpers::dummy_cctx_with_pagination_response(&["page_3_index_1", "page_3_index_2"], "end"),
        ))
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;
    // simulate realtime fetcher getting some relevant data
    Mock::given(method("GET"))
        .and(path("/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            helpers::dummy_cctx_with_pagination_response(
                &["page_1_index_1", "page_1_index_2"],
                "SECOND_PAGE",
            ),
        ))
        .mount(&mock_server)
        .await;



    

    //supress status update/related cctx search
    Mock::given(method("GET"))
        .and(path_regex(r"/crosschain/cctx/.+"))
        .respond_with(ResponseTemplate::new(200).set_body_json( 
            json!({
                "CrossChainTx": helpers::dummy_cross_chain_tx("page_3_index_1", "OutboundMined")
            })
        ))
        .mount(&mock_server)
        .await;
    //supress status update/related cctx search
    Mock::given(method("GET"))
        .and(path_regex(r"crosschain/inboundHashToCctxData/.+"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(
                    json!({
                        "CrossChainTxs": []
                    })
                ),
        )
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
            concurrency: 10,
            realtime_threshold: 10_000,
            enabled: true,
            ..Default::default()
        },
        Arc::new(rpc_client),
        Arc::new(ZetachainCctxDatabase::new(db.client().clone())),
        Arc::new(NoOpBroadcaster{}),
    );

    // Run indexer for a short time to process realtime data
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

    tokio::time::sleep(Duration::from_millis(1000)).await;

    indexer_handle.abort();

    let indices = vec![
        "page_1_index_1",
        "page_1_index_2",
        "page_2_index_1",
        "page_2_index_2",
        "page_3_index_1",
        "page_3_index_2",
    ];

    for index in indices {
        let cctx = cross_chain_tx::Entity::find()
            .filter(cross_chain_tx::Column::Index.eq(index))
            .one(db.client().as_ref())
            .await
            .unwrap();
        assert!(cctx.is_some(), "CCTX with index {} not found", index);
        
    }

    let cctx_count = cross_chain_tx::Entity::find()
        .count(db.client().as_ref())
        .await
        .unwrap();

    // 7 cctxs are expected: 1 historical, 6 realtime
    assert_eq!(cctx_count, 7);
}
