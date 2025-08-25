use std::sync::Arc;
use std::time::Duration;
use actix_phoenix_channel::ChannelCentral;
use pretty_assertions::assert_eq;
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use sea_orm::PaginatorTrait;
use serde_json::json;
use wiremock::matchers::path_regex;
use wiremock::{
    matchers::{method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};
use zetachain_cctx_entity::sea_orm_active_enums::ProcessingStatus;
use zetachain_cctx_entity::{cross_chain_tx, sea_orm_active_enums::Kind, watermark};

mod helpers;

use zetachain_cctx_logic::channel::Channel;
use zetachain_cctx_logic::{
    client::{Client, RpcSettings},
    database::ZetachainCctxDatabase,
    indexer::Indexer,
    settings::IndexerSettings,
};


#[tokio::test]
async fn test_level_data_gap() {
    
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        helpers::init_tests_logs().await;
    }
    let db = crate::helpers::init_db("test", "indexer_level_data_gap").await;

    let database = ZetachainCctxDatabase::new(db.client().clone(), 7001);

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
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "end"))
        .respond_with(ResponseTemplate::new(200).set_body_json(helpers::empty_cctx_response()))
        .mount(&mock_server)
        .await;


        Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
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
        .and(path("/zeta-chain/crosschain/cctx"))
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
        .and(path("/zeta-chain/crosschain/cctx"))
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
        .and(path_regex(r"/zeta-chain/crosschain/cctx/.+"))
        .respond_with(ResponseTemplate::new(200).set_body_json( 
            json!({
                "CrossChainTx": helpers::dummy_cross_chain_tx("page_3_index_1", "OutboundMined")
            })
        ))
        .mount(&mock_server)
        .await;
    //supress status update/related cctx search
    Mock::given(method("GET"))
        .and(path_regex(r"/zeta-chain/crosschain/inboundHashToCctxData/.+"))
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

    let channel = Arc::new(ChannelCentral::new(Channel));
    let indexer = Indexer::new(
        IndexerSettings {
            polling_interval: 100, // Fast polling for tests
            concurrency: 10,
            realtime_threshold: 10_000,
            enabled: true,
            ..Default::default()
        },
        Arc::new(rpc_client),
        Arc::new(database),
        Arc::new(channel.channel_broadcaster()),
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
            tracing::info!("Indexer timed out");
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
        assert!(cctx.is_some(), "CCTX with index {index} not found");
        
    }

    let cctx_count = cross_chain_tx::Entity::find()
        .count(db.client().as_ref())
        .await
        .unwrap();

    // 7 cctxs are expected: 1 historical, 6 realtime
    assert_eq!(cctx_count, 6);
}
