mod helpers;
use futures::StreamExt;

use serde_json::json;
use std::{sync::Arc, time::Duration};

use uuid::Uuid;
use wiremock::{
    matchers::{method, path, path_regex, query_param},
    Mock, MockServer, ResponseTemplate,
};
use zetachain_cctx_logic::{
    client::{Client, RpcSettings},
    database::ZetachainCctxDatabase,
    models::CoinType
};

use crate::helpers::{dummy_cross_chain_tx, dummy_token};
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::CctxListItem;

#[tokio::test]
#[ignore = "Needs database to run"]
async fn emit_imported_cctxs() {
    let db = crate::helpers::init_db("test", "websocket").await;
    let database = Arc::new(ZetachainCctxDatabase::new(db.client().clone(), 7001));
    let zeta_token = dummy_token("Test Token", "TEST", None, "1", CoinType::Zeta);
    let erc20_token = dummy_token(
        "Test Token",
        "TEST",
        Some("0x123".to_string()),
        "1",
        CoinType::ERC20,
    );
    let gas_token = dummy_token("Test Token", "TEST", None, "1", CoinType::Gas);

    database
        .sync_tokens(Uuid::new_v4(), vec![zeta_token.clone(), erc20_token.clone(), gas_token.clone()])
        .await
        .unwrap();

    let mock_server = MockServer::start().await;

    let dummy_cctxs = [zeta_token, erc20_token, gas_token]
        .iter()
        .map(|token| {
            let mut cctx = dummy_cross_chain_tx("dummy_cctx", "PendingOutbound");
            cctx.inbound_params.coin_type = token.coin_type.clone();
            cctx.inbound_params.asset = token.asset.clone();
            cctx.inbound_params.sender_chain_id = token.foreign_chain_id.clone();
            cctx
        })
        .collect::<Vec<_>>();

    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "CrossChainTx": dummy_cctxs,
            "pagination": {
                "next_key": "",
                "total": "3"
            }
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/zeta-chain/crosschain/cctx"))
        .and(query_param("unordered", "true"))
        .and(query_param("pagination.key", "MH=="))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "CrossChainTx": [],
            "pagination": {
                "next_key": "MH==",
                "total": "0"
            }
        })))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex("/zeta-chain/crosschain/cctx/.+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "CrossChainTx": dummy_cctxs[0]
        })))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex("/zeta-chain/crosschain/inboundHashToCctxData/.+"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "CrossChainTxs": []
        })))
        .mount(&mock_server)
        .await;
    let client = Client::new(RpcSettings {
        url: mock_server.uri().to_string(),
        ..Default::default()
    });
    // Start the full server with websocket support
    let (_base, channel) = crate::helpers::init_zetachain_cctx_server(
        db.db_url(),
        |mut settings| {
            settings.websocket.enabled = true; // Enable websocket
            settings.indexer.enabled = false; // Disable indexer for this test
            settings.indexer.zetachain_id = 7001;
            settings.indexer.token_sync_enabled = false;
            settings
        },
        db.client(),
        Arc::new(client),
    )
    .await;

    let (_client, mut receiver) = channel.build_client();

    tokio::time::sleep(Duration::from_secs(2)).await;

    let cctxs = receiver.next().await.unwrap();
    
    let cctxs: Vec<CctxListItem> = serde_json::from_str(&cctxs).unwrap();

    assert_eq!(cctxs.len(), 3);
}
