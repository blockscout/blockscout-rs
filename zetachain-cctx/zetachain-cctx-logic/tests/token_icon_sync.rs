use std::{sync::Arc, time::Duration};

mod helpers;

use actix_phoenix_channel::ChannelCentral;
use pretty_assertions::assert_eq;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use wiremock::{
    matchers::{method, path, path_regex},
    Mock, MockServer, ResponseTemplate,
};

use zetachain_cctx_entity::token;
use zetachain_cctx_logic::{
    channel::Channel,
    client::{Client, RpcSettings},
    database::ZetachainCctxDatabase,
    indexer::Indexer,
    models::CctxStatusStatus,
    settings::IndexerSettings,
};

fn dummy_token_response() -> serde_json::Value {
    json!({
        "foreignCoins": [
            {
                "zrc20_contract_address": "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
                "asset": "0xasset",
                "foreign_chain_id": "1",
                "decimals": 18,
                "name": "Dummy Token",
                "symbol": "DUM",
                "coin_type": "ERC20",
                "gas_limit": "100000",
                "paused": false,
                "liquidity_cap": "1000000000"
            }
        ],
        "pagination": { "next_key": null, "total": "0" }
    })
}

#[tokio::test]
async fn test_icon_url_synced() {
    if std::env::var("TEST_TRACING").unwrap_or_default() == "true" {
        helpers::init_tests_logs().await;
    }
    let db = helpers::init_db("test", "icon_url_synced").await;

    let api_server = MockServer::start().await;
    let blockscout_server = MockServer::start().await;

    // Mock token list response
    Mock::given(method("GET"))
        .and(path("/zeta-chain/fungible/foreign_coins"))
        .respond_with(ResponseTemplate::new(200).set_body_json(dummy_token_response()))
        .mount(&api_server)
        .await;

    // Mock cctx endpoints to noop
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .respond_with(ResponseTemplate::new(200).set_body_json(helpers::empty_cctx_response()))
        .mount(&api_server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/zeta-chain/crosschain/cctx/.+"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(helpers::dummy_cross_chain_tx(
                "dummy",
                CctxStatusStatus::OutboundMined,
            )),
        )
        .mount(&api_server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(
            r"/zeta-chain/crosschain/inboundHashToCctxData/.+",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"CrossChainTxs": []})))
        .mount(&api_server)
        .await;

    // Mock blockscout token icon endpoint
    Mock::given(method("GET"))
        .and(path(
            "/api/v2/tokens/0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "icon_url": "https://example.com/icon.png" })),
        )
        .mount(&blockscout_server)
        .await;

    let client = Client::new(RpcSettings {
        url: api_server.uri().to_string(),
        blockscout_instance_url: blockscout_server.uri().to_string() + "/", // ensure trailing slash
        request_per_second: 100,
        ..Default::default()
    });

    let database = ZetachainCctxDatabase::new(db.client(), 7001);
    database.setup_db().await.unwrap();

    let channel = Arc::new(ChannelCentral::new(Channel));
    let indexer = Indexer::new(
        IndexerSettings {
            polling_interval: 50,
            token_polling_interval: 50,
            token_batch_size: 100,
            concurrency: 1,
            enabled: true,
            ..Default::default()
        },
        Arc::new(client),
        Arc::new(database),
        Arc::new(channel.channel_broadcaster()),
    );

    // Run indexer briefly
    let handle = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_millis(300), async {
            let _ = indexer.run().await;
        })
        .await
        .ok();
    });

    tokio::time::sleep(Duration::from_millis(400)).await;
    handle.abort();

    // Check DB for icon_url
    let conn = db.client();
    let stored = token::Entity::find()
        .filter(
            token::Column::Zrc20ContractAddress.eq("0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"),
        )
        .one(conn.as_ref())
        .await
        .unwrap()
        .expect("token");
    assert_eq!(
        stored.icon_url,
        Some("https://example.com/icon.png".to_string())
    );
}
